use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;

pub(crate) trait Node {
    type Id;

    fn id(&self) -> Self::Id;

    fn table_header() -> String;

    fn table_data(&self) -> String;

    fn node_header() -> String;

    fn format_header(width: usize) -> Vec<String> {
        let mut first = String::new();
        first += &Self::table_header();
        first += " ┃ ";
        first += &Self::node_header();
        let mut second = String::new();
        second += &"━".repeat(Self::table_header().len() + 1);
        second += "╋";
        second += &"━".repeat(width.saturating_sub(Self::table_header().len() + 2));
        vec![first, second]
    }

    fn parent(&self) -> Option<Self::Id>;

    fn cmp(&self, other: &Self) -> Ordering;

    fn accumulate_from(&mut self, other: &Self);
}

#[derive(Debug)]
pub(crate) struct Tree<Node> {
    node: Node,
    children: Forest<Node>,
}

#[derive(Debug)]
pub(crate) struct Forest<Node>(Vec<Tree<Node>>);

impl<Node> Forest<Node>
where
    Node: crate::tree::Node + Display,
    Node::Id: Hash + Eq + Copy,
{
    pub(crate) fn new_forest(input: impl Iterator<Item = Node>) -> Self {
        let mut node_map = HashMap::new();
        let mut children_map = HashMap::new();
        let mut roots = Vec::new();
        for node in input {
            if let Some(parent) = node.parent() {
                children_map
                    .entry(parent)
                    .or_insert(Vec::new())
                    .push(node.id());
            } else {
                roots.push(node.id());
            }
            node_map.insert(node.id(), node);
        }
        let mut result = Forest::mk_forest(&mut node_map, &mut children_map, roots);
        result.compute_accumulate();
        result.sort();
        result
    }

    fn mk_forest(
        node_map: &mut HashMap<Node::Id, Node>,
        children_map: &mut HashMap<Node::Id, Vec<Node::Id>>,
        roots: Vec<Node::Id>,
    ) -> Self {
        let mut result = Forest(Vec::new());
        for root in roots.into_iter() {
            let children = children_map.remove(&root).unwrap_or_default();
            result.0.push(Tree {
                node: node_map.remove(&root).unwrap(),
                children: Forest::mk_forest(node_map, children_map, children),
            });
        }
        result
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Node> {
        struct Iter<'a, Node>(VecDeque<&'a Tree<Node>>);

        impl<'a, Node> Iterator for Iter<'a, Node> {
            type Item = &'a Node;

            fn next(&mut self) -> Option<&'a Node> {
                match self.0.pop_front() {
                    Some(tree) => {
                        for child in tree.children.0.iter().rev() {
                            self.0.push_front(child);
                        }
                        Some(&tree.node)
                    }
                    None => None,
                }
            }
        }

        Iter(self.0.iter().rev().collect())
    }

    fn sort(&mut self) {
        self.0.sort_by(|a, b| a.node.cmp(&b.node));
        for tree in self.0.iter_mut() {
            tree.children.sort();
        }
    }

    fn compute_accumulate(&mut self) {
        for tree in self.0.iter_mut() {
            tree.children.compute_accumulate();
            for child in tree.children.0.iter_mut() {
                tree.node.accumulate_from(&child.node);
            }
        }
    }

    fn filter<F>(&self, filter: F) -> HashSet<Node::Id>
    where
        F: Fn(&Node) -> bool,
    {
        let mut result = HashSet::new();
        self.filter_helper(&filter, false, &mut result);
        result
    }

    fn filter_helper<F>(
        &self,
        filter: &F,
        parent_included: bool,
        included: &mut HashSet<Node::Id>,
    ) -> bool
    where
        F: Fn(&Node) -> bool,
    {
        let mut any_child_included = false;
        for tree in self.0.iter() {
            if parent_included || filter(&tree.node) {
                included.insert(tree.node.id());
                tree.children.filter_helper(filter, true, included);
                any_child_included = true
            } else if tree.children.filter_helper(filter, false, included) {
                included.insert(tree.node.id());
                any_child_included = true;
            }
        }
        any_child_included
    }

    pub(crate) fn format_processes<F>(&self, filter: F) -> Vec<(Node::Id, String)>
    where
        F: Fn(&Node) -> bool,
    {
        let included = self.filter(filter);
        let mut acc = Vec::new();
        self.format_helper(&included, true, &mut Vec::new(), &mut acc);
        acc
    }

    fn format_helper(
        &self,
        included: &HashSet<Node::Id>,
        is_root: bool,
        prefixes: &mut Vec<&str>,
        acc: &mut Vec<(Node::Id, String)>,
    ) {
        let children: Vec<&Tree<Node>> = self
            .0
            .iter()
            .filter(|child| included.contains(&child.node.id()))
            .collect();
        for (i, child) in children.iter().enumerate() {
            let is_last = i == children.len() - 1;
            if !included.contains(&child.node.id()) {
                continue;
            }
            let mut line = String::new();
            line += &format!("{} ┃ ", child.node.table_data());
            for prefix in prefixes.iter() {
                line += prefix;
            }
            if !is_root {
                line += if is_last { "└─" } else { "├─" };
                let has_children = !child.children.0.is_empty();
                line += if has_children { "┬ " } else { "─ " };
            }
            line += &format!("{}", child.node);
            acc.push((child.node.id(), line));
            if !(is_root) {
                prefixes.push(if is_last { "  " } else { "│ " });
            }
            child.children.format_helper(included, false, prefixes, acc);
            prefixes.pop();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use unindent::Unindent;

    impl<Node> Forest<Node>
    where
        Node: Display,
        Node: crate::tree::Node,
        Node::Id: Eq + Copy + Hash,
    {
        fn test_format<F>(&self, filter: F, width: u16) -> String
        where
            F: Fn(&Node) -> bool,
        {
            let header = Node::format_header(width.into());
            let table: Vec<String> = self
                .format_processes(filter)
                .into_iter()
                .map(|x| x.1)
                .collect();
            format!("{}\n{}\n", header.join("\n"), table.join("\n"))
        }
    }

    #[derive(Debug)]
    struct TestNode {
        id: u8,
        parent: Option<u8>,
    }

    impl Display for TestNode {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}",
                match self.id {
                    1 => "one",
                    2 => "two",
                    3 => "three",
                    4 => "four",
                    n => panic!("TestNode out of range: {}", n),
                }
            )
        }
    }

    impl Node for TestNode {
        type Id = u8;

        fn id(&self) -> u8 {
            self.id
        }

        fn table_header() -> String {
            "#".to_owned()
        }

        fn table_data(&self) -> String {
            self.id.to_string()
        }

        fn node_header() -> String {
            "number".to_string()
        }

        fn parent(&self) -> Option<u8> {
            self.parent
        }

        fn cmp(&self, other: &Self) -> Ordering {
            self.id.cmp(&other.id)
        }

        fn accumulate_from(&mut self, _other: &Self) {}
    }

    impl TestNode {
        fn new(id: u8, parent: Option<u8>) -> TestNode {
            TestNode { id, parent }
        }
    }

    #[test]
    fn a_single_node_tree() {
        let tree = Forest::new_forest(vec![TestNode::new(1, None)].into_iter());
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
            "
            .unindent()
        );
    }

    #[test]
    fn b_child() {
        let tree =
            Forest::new_forest(vec![TestNode::new(1, None), TestNode::new(2, Some(1))].into_iter());
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
                2 ┃ └── two
            "
            .unindent()
        );
    }

    #[test]
    fn c_children() {
        let tree = Forest::new_forest(
            vec![
                TestNode::new(1, None),
                TestNode::new(2, Some(1)),
                TestNode::new(3, Some(1)),
                TestNode::new(4, Some(1)),
            ]
            .into_iter(),
        );
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
                2 ┃ ├── two
                3 ┃ ├── three
                4 ┃ └── four
            "
            .unindent()
        );
    }

    #[test]
    fn d_grandchildren() {
        let tree = Forest::new_forest(
            vec![
                TestNode::new(1, None),
                TestNode::new(2, Some(1)),
                TestNode::new(3, Some(2)),
            ]
            .into_iter(),
        );
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
                2 ┃ └─┬ two
                3 ┃   └── three
            "
            .unindent()
        );
    }

    #[test]
    fn e_bigger() {
        let tree = Forest::new_forest(
            vec![
                TestNode::new(1, None),
                TestNode::new(2, Some(1)),
                TestNode::new(3, Some(2)),
                TestNode::new(4, Some(1)),
            ]
            .into_iter(),
        );
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
                2 ┃ ├─┬ two
                3 ┃ │ └── three
                4 ┃ └── four
            "
            .unindent()
        );
    }

    #[test]
    fn f_multiple_roots() {
        let tree =
            Forest::new_forest(vec![TestNode::new(1, None), TestNode::new(2, None)].into_iter());
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
                2 ┃ two
            "
            .unindent()
        );
    }

    #[test]
    fn g_sorts_roots_by_id() {
        let tree =
            Forest::new_forest(vec![TestNode::new(2, None), TestNode::new(1, None)].into_iter());
        assert_eq!(
            tree.test_format(|_| true, 25),
            "
                # ┃ number
                ━━╋━━━━━━━━━━━━━━━━━━━━━━
                1 ┃ one
                2 ┃ two
            "
            .unindent()
        );
    }

    mod h_filtering {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn a_filters_nodes() {
            let tree = Forest::new_forest(
                vec![TestNode::new(1, None), TestNode::new(2, None)].into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 2, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    2 ┃ two
                "
                .unindent()
            );
        }

        #[test]
        fn b_shows_children_of_included_nodes() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, None),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 1, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ one
                    2 ┃ └── two
                "
                .unindent()
            );
        }

        #[test]
        fn c_shows_parents_of_included_nodes() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, None),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 2, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ one
                    2 ┃ └── two
                "
                .unindent()
            );
        }

        #[test]
        fn d_shows_transitive_parents() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 3, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ one
                    2 ┃ └─┬ two
                    3 ┃   └── three
                "
                .unindent()
            );
        }

        #[test]
        fn e_bigger() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                    TestNode::new(4, None),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 2, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ one
                    2 ┃ └─┬ two
                    3 ┃   └── three
                "
                .unindent()
            );
        }

        #[test]
        fn f_no_unconnected_lines() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                    TestNode::new(4, Some(1)),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 2, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ one
                    2 ┃ └─┬ two
                    3 ┃   └── three
                "
                .unindent()
            );
        }
    }

    mod i_accumulation {
        use crate::tree::{Forest, Node};
        use pretty_assertions::assert_eq;
        use std::fmt::Display;
        use unindent::Unindent;

        #[derive(Debug)]
        struct TestNode {
            id: u8,
            parent: Option<u8>,
            to_accumulate: i32,
        }

        impl TestNode {
            fn new(id: u8, parent: Option<u8>, to_accumulate: i32) -> Self {
                TestNode {
                    id,
                    parent,
                    to_accumulate,
                }
            }
        }

        impl Display for TestNode {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_accumulate)
            }
        }

        impl Node for TestNode {
            type Id = u8;

            fn id(&self) -> u8 {
                self.id
            }

            fn table_header() -> String {
                "#".to_owned()
            }

            fn table_data(&self) -> String {
                self.id.to_string()
            }

            fn node_header() -> String {
                "number".to_owned()
            }

            fn parent(&self) -> Option<u8> {
                self.parent
            }

            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                other.to_accumulate.cmp(&self.to_accumulate)
            }

            fn accumulate_from(&mut self, other: &Self) {
                self.to_accumulate += other.to_accumulate;
            }
        }

        #[test]
        fn a_can_compute_accumulated_values_of_children() {
            let tree = Forest::new_forest(
                vec![TestNode::new(1, None, 2), TestNode::new(2, Some(1), 3)].into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 2, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ 5
                    2 ┃ └── 3
                "
                .unindent()
            );
        }

        #[test]
        fn b_accumulates_from_grandchildren() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None, 2),
                    TestNode::new(2, Some(1), 3),
                    TestNode::new(3, Some(2), 8),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|node| node.id == 2, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ 13
                    2 ┃ └─┬ 11
                    3 ┃   └── 8
                "
                .unindent()
            );
        }

        #[test]
        fn c_sorting_happens_after_accumulation() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None, 0),
                    TestNode::new(2, Some(1), 1),
                    TestNode::new(3, Some(2), 4),
                    TestNode::new(4, Some(1), 2),
                    TestNode::new(5, Some(4), 2),
                    TestNode::new(6, Some(1), 3),
                    TestNode::new(7, Some(6), 0),
                ]
                .into_iter(),
            );
            assert_eq!(
                tree.test_format(|_| true, 25),
                "
                    # ┃ number
                    ━━╋━━━━━━━━━━━━━━━━━━━━━━
                    1 ┃ 12
                    2 ┃ ├─┬ 5
                    3 ┃ │ └── 4
                    4 ┃ ├─┬ 4
                    5 ┃ │ └── 2
                    6 ┃ └─┬ 3
                    7 ┃   └── 0
                "
                .unindent()
            );
        }
    }

    mod j_headers {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn a_renders_headers() {
            struct N;
            impl Node for N {
                type Id = u64;

                fn id(&self) -> Self::Id {
                    todo!()
                }

                fn table_header() -> String {
                    "a b c".to_owned()
                }

                fn table_data(&self) -> String {
                    todo!()
                }

                fn node_header() -> String {
                    "node header".to_owned()
                }

                fn parent(&self) -> Option<Self::Id> {
                    todo!()
                }

                fn cmp(&self, _other: &Self) -> Ordering {
                    todo!()
                }

                fn accumulate_from(&mut self, _other: &Self) {
                    todo!()
                }
            }

            assert_eq!(
                format!("{}\n", N::format_header(25).join("\n")),
                "
                    a b c ┃ node header
                    ━━━━━━╋━━━━━━━━━━━━━━━━━━
                "
                .unindent()
            );
        }
    }

    mod k_iterators {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn a_iterates_through_all_the_nodes() {
            let tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                    TestNode::new(4, Some(1)),
                ]
                .into_iter(),
            );
            eprintln!("{}", tree.test_format(|_| true, 25));
            assert_eq!(
                tree.iter().map(Node::id).collect::<Vec<u8>>(),
                vec![1, 2, 3, 4]
            );
        }
    }
}
