extern crate regex;

use crate::detections::configs;
use regex::Regex;
use serde_json::Value;
use yaml_rust::Yaml;

pub fn parse_rule(yaml: Yaml) -> RuleNode {
    let detection = parse_detection(&yaml);

    return RuleNode {
        yaml: yaml,
        detection: detection,
    };
}

fn parse_detection(yaml: &Yaml) -> Option<DetectionNode> {
    if yaml["detection"].is_badvalue() {
        return Option::None;
    } else {
        let node = DetectionNode {
            selection: parse_selection(&yaml),
        };
        return Option::Some(node);
    }
}

pub fn get_event_value<'a>(key: &String, event_value: &'a Value) -> Option<&'a Value> {
    if key.len() == 0 {
        return Option::None;
    }

    let alias_config = configs::singleton().event_key_alias_config;
    let event_key = match alias_config.get_event_key(key.to_string()) {
        Some(alias_event_key) => alias_event_key,
        None => key,
    };

    let mut ret: &Value = event_value;
    for key in event_key.split(".") {
        if ret.is_object() == false {
            return Option::None;
        }
        ret = &ret[key];
    }

    return Option::Some(ret);
}

fn concat_selection_key(key_list: &Vec<String>) -> String {
    return key_list
        .iter()
        .fold("detection -> selection".to_string(), |mut acc, cur| {
            acc = acc + " -> " + cur;
            return acc;
        });
}

fn parse_selection(yaml: &Yaml) -> Option<Box<dyn SelectionNode>> {
    // TODO detection-selectionが存在しない場合のチェック
    let selection_yaml = &yaml["detection"]["selection"];
    if selection_yaml.is_badvalue() {
        return Option::None;
    }
    return Option::Some(parse_selection_recursively(vec![], &selection_yaml));
}

fn parse_selection_recursively(key_list: Vec<String>, yaml: &Yaml) -> Box<dyn SelectionNode> {
    if yaml.as_hash().is_some() {
        // 連想配列はAND条件と解釈する
        let yaml_hash = yaml.as_hash().unwrap();
        let mut and_node = AndSelectionNode::new();

        yaml_hash.keys().for_each(|hash_key| {
            let child_yaml = yaml_hash.get(hash_key).unwrap();
            let mut child_key_list = key_list.clone();
            child_key_list.push(hash_key.as_str().unwrap().to_string());
            let child_node = parse_selection_recursively(child_key_list, child_yaml);
            and_node.child_nodes.push(child_node);
        });
        return Box::new(and_node);
    } else if yaml.as_vec().is_some() {
        // 配列はOR条件と解釈する。
        let mut or_node = OrSelectionNode::new();
        yaml.as_vec().unwrap().iter().for_each(|child_yaml| {
            let child_node = parse_selection_recursively(key_list.clone(), child_yaml);
            or_node.child_nodes.push(child_node);
        });

        return Box::new(or_node);
    } else {
        // 連想配列と配列以外は末端ノード
        return Box::new(LeafSelectionNode::new(key_list, yaml.clone()));
    }
}

// Ruleファイルを表すノード
pub struct RuleNode {
    pub yaml: Yaml,
    detection: Option<DetectionNode>,
}

impl RuleNode {
    pub fn init(&mut self) -> Result<(), Vec<String>> {
        if self.detection.is_none() {
            return Result::Ok(());
        }

        return self.detection.as_mut().unwrap().init();
    }

    pub fn select(&self, event_record: &Value) -> bool {
        let selection = self
            .detection
            .as_ref()
            .and_then(|detect_node| detect_node.selection.as_ref());
        if selection.is_none() {
            return false;
        }

        return selection.unwrap().select(event_record);
    }
}

// Ruleファイルのdetectionを表すノード
struct DetectionNode {
    pub selection: Option<Box<dyn SelectionNode>>,
}

impl DetectionNode {
    fn init(&mut self) -> Result<(), Vec<String>> {
        if self.selection.is_none() {
            return Result::Ok(());
        }

        return self.selection.as_mut().unwrap().init();
    }
}

// Ruleファイルの detection- selection配下のノードはこのtraitを実装する。
trait SelectionNode {
    fn select(&self, event_record: &Value) -> bool;
    fn init(&mut self) -> Result<(), Vec<String>>;
}

// detection - selection配下でAND条件を表すノード
struct AndSelectionNode {
    pub child_nodes: Vec<Box<dyn SelectionNode>>,
}

impl AndSelectionNode {
    pub fn new() -> AndSelectionNode {
        return AndSelectionNode {
            child_nodes: vec![],
        };
    }
}

impl SelectionNode for AndSelectionNode {
    fn select(&self, event_record: &Value) -> bool {
        return self.child_nodes.iter().all(|child_node| {
            return child_node.as_ref().select(event_record);
        });
    }

    fn init(&mut self) -> Result<(), Vec<String>> {
        let err_msgs = self
            .child_nodes
            .iter_mut()
            .map(|child_node| {
                let res = child_node.init();
                if res.is_err() {
                    return res.unwrap_err();
                } else {
                    return vec![];
                }
            })
            .fold(
                vec![],
                |mut acc: Vec<String>, cur: Vec<String>| -> Vec<String> {
                    acc.extend(cur.into_iter());
                    return acc;
                },
            );

        if err_msgs.is_empty() {
            return Result::Ok(());
        } else {
            return Result::Err(err_msgs);
        }
    }
}

// detection - selection配下でOr条件を表すノード
struct OrSelectionNode {
    pub child_nodes: Vec<Box<dyn SelectionNode>>,
}

impl OrSelectionNode {
    pub fn new() -> OrSelectionNode {
        return OrSelectionNode {
            child_nodes: vec![],
        };
    }
}

impl SelectionNode for OrSelectionNode {
    fn select(&self, event_record: &Value) -> bool {
        return self.child_nodes.iter().any(|child_node| {
            return child_node.as_ref().select(event_record);
        });
    }

    fn init(&mut self) -> Result<(), Vec<String>> {
        let err_msgs = self
            .child_nodes
            .iter_mut()
            .map(|child_node| {
                let res = child_node.init();
                if res.is_err() {
                    return res.unwrap_err();
                } else {
                    return vec![];
                }
            })
            .fold(
                vec![],
                |mut acc: Vec<String>, cur: Vec<String>| -> Vec<String> {
                    acc.extend(cur.into_iter());
                    return acc;
                },
            );

        if err_msgs.is_empty() {
            return Result::Ok(());
        } else {
            return Result::Err(err_msgs);
        }
    }
}

// detection - selection配下の末端ノード
struct LeafSelectionNode {
    key_list: Vec<String>,
    select_value: Yaml,
    matcher: Option<Box<dyn LeafMatcher>>,
}

impl LeafSelectionNode {
    fn new(key_list: Vec<String>, value_yaml: Yaml) -> LeafSelectionNode {
        return LeafSelectionNode {
            key_list: key_list,
            select_value: value_yaml,
            matcher: Option::None,
        };
    }

    // JSON形式のEventJSONから値を取得する関数 aliasも考慮されている。
    fn get_event_value<'a>(&self, event_value: &'a Value) -> Option<&'a Value> {
        if self.key_list.is_empty() {
            return Option::None;
        }

        return get_event_value(&self.key_list[0].to_string(), event_value);
    }

    // LeafMatcherの一覧を取得する。
    fn get_matchers(&self) -> Vec<Box<dyn LeafMatcher>> {
        return vec![Box::new(RegexMatcher::new())];
    }

    // LeafMatcherを取得する。
    fn get_matcher(&self) -> Option<Box<dyn LeafMatcher>> {
        let matchers = self.get_matchers();
        let mut match_key_list = self.key_list.clone();
        match_key_list.remove(0);
        return matchers
            .into_iter()
            .find(|matcher| matcher.is_target_key(&match_key_list));
    }
}

impl SelectionNode for LeafSelectionNode {
    fn select(&self, event_record: &Value) -> bool {
        if self.matcher.is_none() {
            return false;
        }

        let event_value = self.get_event_value(event_record);
        return self.matcher.as_ref().unwrap().is_match(event_value);
    }

    fn init(&mut self) -> Result<(), Vec<String>> {
        let matchers = self.get_matchers();
        let mut match_key_list = self.key_list.clone();
        match_key_list.remove(0);
        self.matcher = matchers
            .into_iter()
            .find(|matcher| matcher.is_target_key(&match_key_list));
        // 一致するmatcherが見つからないエラー
        if self.matcher.is_none() {
            return Result::Err(vec![format!(
                "Found unknown key. key:{}",
                concat_selection_key(&match_key_list)
            )]);
        }

        return self
            .matcher
            .as_mut()
            .unwrap()
            .init(&match_key_list, &self.select_value);
    }
}

// 末端ノードがEventLogの値を比較するロジックを表す。
// 正規条件のマッチや文字数制限など、比較ロジック毎にこのtraitを実装したクラスが存在する。
//
// 新規にLeafMatcherを実装するクラスを作成した場合、
// LeafSelectionNodeのget_matchersクラスの戻り値の配列に新規作成したクラスのインスタンスを追加する。
trait LeafMatcher {
    fn is_target_key(&self, key_list: &Vec<String>) -> bool;

    fn is_match(&self, event_value: Option<&Value>) -> bool;

    fn init(&mut self, key_list: &Vec<String>, select_value: &Yaml) -> Result<(), Vec<String>>;
}

// 正規表現で比較するロジックを表すクラス
struct RegexMatcher {
    re: Option<Regex>,
}

impl RegexMatcher {
    fn new() -> RegexMatcher {
        return RegexMatcher {
            re: Option::None, // empty
        };
    }
    fn is_regex_fullmatch(&self, re: &Regex, value: String) -> bool {
        return re.find_iter(&value).any(|match_obj| {
            return match_obj.as_str().to_string() == value;
        });
    }
}

impl LeafMatcher for RegexMatcher {
    fn is_target_key(&self, key_list: &Vec<String>) -> bool {
        return key_list.is_empty();
    }

    fn init(&mut self, key_list: &Vec<String>, select_value: &Yaml) -> Result<(), Vec<String>> {
        if select_value.is_null() {
            self.re = Option::None;
            return Result::Ok(());
        }

        // stringで比較する。
        let yaml_value = match select_value {
            Yaml::Boolean(b) => Option::Some(b.to_string()),
            Yaml::Integer(i) => Option::Some(i.to_string()),
            Yaml::Real(r) => Option::Some(r.to_string()),
            Yaml::String(s) => Option::Some(s.to_owned()),
            _ => Option::None,
        };
        // ここには来ないはず
        if yaml_value.is_none() {
            let errmsg = format!(
                "unknown error occured. [key:{}]",
                concat_selection_key(key_list)
            );
            return Result::Err(vec![errmsg]);
        }

        // 指定された正規表現が間違っていて、パースに失敗した場合
        let yaml_str = yaml_value.unwrap();
        let re_result = Regex::new(&yaml_str);
        if re_result.is_err() {
            let errmsg = format!(
                "cannot parse regex. [regex:{}, key:{}]",
                yaml_str,
                concat_selection_key(key_list)
            );
            return Result::Err(vec![errmsg]);
        }

        return Result::Ok(());
    }

    fn is_match(&self, event_value: Option<&Value>) -> bool {
        let is_event_value_null = event_value.is_none()
            || event_value.unwrap().is_null()
            || event_value.unwrap().as_str().unwrap_or(" ").len() == 0;

        // yamlにnullが設定されていた場合
        if self.re.is_none() {
            return is_event_value_null;
        }

        return match event_value.unwrap_or(&Value::Null) {
            Value::Bool(b) => self.is_regex_fullmatch(self.re.as_ref().unwrap(), b.to_string()),
            Value::String(s) => self.is_regex_fullmatch(self.re.as_ref().unwrap(), s.to_owned()),
            Value::Number(n) => self.is_regex_fullmatch(self.re.as_ref().unwrap(), n.to_string()),
            _ => false,
        };
    }
}
