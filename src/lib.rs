use std::collections::HashMap;

use wasm_bindgen::prelude::*;

type Handler = Option<JsValue>;
type Params = HashMap<String, String>;

const PARAM_PREFIX_DEFAULT: &str = ":";
const PATH_SEPARATOR_DEFAULT: &str = "/";
const WILDCARD_SYMBOL_DEFAULT: char = '*';

enum TreeNode<'a> {
    Static(&'a StaticTreeNode),
    Dynamic(&'a DynamicTreeNode),
}

impl<'a> TreeNode<'a> {
    pub fn extract_static_node(&self) -> &StaticTreeNode {
        match self {
            Self::Static(n) => n,
            Self::Dynamic(n) => &n.node,
        }
    }
}

enum TreeNodeMut<'a> {
    Static(&'a mut StaticTreeNode),
    Dynamic(&'a mut DynamicTreeNode),
}

impl TreeNodeMut<'_> {
    pub fn extract_static_node(&mut self) -> &mut StaticTreeNode {
        match self {
            Self::Static(n) => n,
            Self::Dynamic(n) => &mut n.node,
        }
    }
}

#[derive(Default)]
struct StaticTreeNode {
    handler: Handler,
    wildcard_handler: Handler,

    static_children: HashMap<String, StaticTreeNode>,
    dynamic_child: Option<Box<DynamicTreeNode>>,
}

struct DynamicTreeNode {
    node: StaticTreeNode,
    param_name: String,
}

impl DynamicTreeNode {
    pub fn new(handler: Handler, param_name: &str) -> Self {
        DynamicTreeNode {
            node: StaticTreeNode::new(handler),
            param_name: param_name.to_string(),
        }
    }
}

impl StaticTreeNode {
    pub fn new(handler: Handler) -> Self {
        StaticTreeNode {
            handler,
            ..Default::default()
        }
    }

    pub fn add_static_child(&mut self, segment: &str, handler: Handler) {
        let child = StaticTreeNode::new(handler);

        self.static_children.insert(segment.to_string(), child);
    }

    pub fn delete_static_child(&mut self, segment: &str) -> Option<StaticTreeNode> {
        self.static_children.remove(segment)
    }

    pub fn set_dynamic_child(&mut self, param_name: &str, handler: Handler) {
        let child = DynamicTreeNode::new(handler, param_name);

        self.dynamic_child = Some(Box::new(child));
    }

    pub fn delete_dynamic_child(&mut self) {
        self.dynamic_child = None
    }

    pub fn set_wildcard_handler(&mut self, handler: Handler) {
        self.wildcard_handler = handler
    }

    pub fn delete_wildcard_handler(&mut self) {
        self.wildcard_handler = None
    }

    pub fn get_static_child(&self, segment: &str) -> Option<&StaticTreeNode> {
        self.static_children.get(segment)
    }

    pub fn has_static_child(&self, segment: &str) -> bool {
        self.static_children.contains_key(segment)
    }

    pub fn get_dynamic_child(&self) -> Option<&DynamicTreeNode> {
        self.dynamic_child.as_ref().map(|n| n.as_ref())
    }

    pub fn has_dynamic_child(&self) -> bool {
        match self.dynamic_child {
            Some(_) => true,
            None => false,
        }
    }

    pub fn get_child(&self, segment: &str) -> Option<TreeNode> {
        self.get_static_child(segment)
            .map(|n| TreeNode::Static(&n))
            .or_else(|| self.get_dynamic_child().map(|n| TreeNode::Dynamic(n)))
    }

    pub fn get_static_child_mut(&mut self, segment: &str) -> Option<&mut StaticTreeNode> {
        self.static_children.get_mut(segment)
    }

    pub fn get_dynamic_child_mut(&mut self) -> Option<&mut DynamicTreeNode> {
        self.dynamic_child.as_mut().map(|n| n.as_mut())
    }

    pub fn get_child_mut(&mut self, segment: &str) -> Option<TreeNodeMut> {
        let static_child = self
            .static_children
            .get_mut(segment)
            .map(|c| TreeNodeMut::Static(c));

        if let Some(static_child) = static_child {
            Some(static_child)
        } else {
            self.dynamic_child.as_mut().map(|n| TreeNodeMut::Dynamic(n))
        }
    }
}

struct TraversePathReturn<'a> {
    node: &'a StaticTreeNode,
    params: Params,
}

impl TraversePathReturn<'_> {
    pub fn extract_handler(&self) -> Option<HandlerAndParams> {
        self.node.handler.as_ref().map(|handler| HandlerAndParams {
            handler: handler.clone(),
            params: serde_wasm_bindgen::to_value(&self.params).unwrap(),
        })
    }
}

fn js_value_to_option(js_value: JsValue) -> Handler {
    if js_value.is_undefined() || js_value.is_null() {
        None
    } else {
        Some(js_value)
    }
}

#[wasm_bindgen]
struct HandlerAndParams {
    handler: JsValue,
    params: JsValue,
}

#[wasm_bindgen]
impl HandlerAndParams {
    #[wasm_bindgen(getter)]
    pub fn handler(&self) -> JsValue {
        self.handler.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn params(&self) -> JsValue {
        self.params.clone()
    }
}

#[wasm_bindgen]
struct RouterTree {
    root: StaticTreeNode,
    path_separator: String,
    param_prefix: String,
    wildcard_symbol: String,
}

#[wasm_bindgen]
impl RouterTree {
    #[wasm_bindgen(constructor)]
    pub fn new(
        handler: JsValue,
        param_prefix: Option<String>,
        path_separator: Option<String>,
        wildcard_symbol: Option<String>,
    ) -> Self {
        let root = StaticTreeNode::new(js_value_to_option(handler));

        RouterTree {
            root,
            path_separator: path_separator.unwrap_or(PATH_SEPARATOR_DEFAULT.to_string()),
            param_prefix: param_prefix.unwrap_or(PARAM_PREFIX_DEFAULT.to_string()),
            wildcard_symbol: wildcard_symbol.unwrap_or(WILDCARD_SYMBOL_DEFAULT.to_string()),
        }
    }

    #[wasm_bindgen]
    pub fn add(&mut self, path: String, handler: JsValue) {
        let segments = self.parse_path(&path);
        let param_prefix = self.param_prefix.as_str();
        let mut current_node = &mut self.root;

        for segment in segments {
            current_node = if RouterTree::is_dynamic_segment(segment, param_prefix) {
                let param_name = RouterTree::strip_param_prefix(segment, param_prefix);

                if !current_node.has_dynamic_child() {
                    current_node.set_dynamic_child(param_name, None);
                }
                &mut current_node.get_dynamic_child_mut().unwrap().node
            } else {
                if !current_node.has_static_child(segment) {
                    current_node.add_static_child(segment, None);
                }
                current_node.get_static_child_mut(segment).unwrap()
            }
        }

        current_node.handler = js_value_to_option(handler);
    }

    #[wasm_bindgen]
    pub fn get(&self, path: String) -> Option<HandlerAndParams> {
        self.traverse_path(&path)
            .map(|v| v.extract_handler())
            .flatten()
    }

    fn traverse_path(&self, path: &String) -> Option<TraversePathReturn> {
        let segments = self.parse_path(path);
        let mut params: Params = HashMap::new();
        let mut current_node = &self.root;

        for segment in segments {
            if current_node.wildcard_handler.is_some() {
                break;
            }
            if let Some(child) = current_node.get_child(segment) {
                match child {
                    TreeNode::Static(node) => current_node = node,
                    TreeNode::Dynamic(node) => {
                        params.insert(node.param_name.clone(), segment.to_string());
                        current_node = &node.node;
                    }
                }
            } else {
                return None;
            }
        }

        return Some(TraversePathReturn {
            node: current_node,
            params,
        });
    }

    fn parse_path<'a>(&self, path: &'a String) -> Vec<&'a str> {
        path.trim_start_matches(&self.path_separator)
            .split(&self.path_separator)
            .collect()
    }

    fn get_root_node(&self) -> TreeNode {
        TreeNode::Static(&self.root)
    }

    fn is_dynamic_segment(segment: &str, param_prefix: &str) -> bool {
        segment.starts_with(param_prefix)
    }

    fn strip_param_prefix<'a>(segment: &'a str, param_prefix: &str) -> &'a str {
        segment.strip_prefix(param_prefix).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut router = RouterTree::new(JsValue::null(), None, None, None);

        router.add("/user/:id".to_string(), JsValue::null());

        let result = router.get("/user/123".to_string());

        dbg!("I am here!!!");

        assert_eq!(1, 1)
    }
}
