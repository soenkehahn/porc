use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;

pub(crate) trait Node {
    type Id;

    fn id(&self) -> Self::Id;

    fn parent(&self) -> Option<Self::Id>;

    fn accumulate_from(&mut self, other: &Self);
}

#[derive(Debug)]
pub(crate) struct Tree<Node> {
    pub(crate) node: Node,
    pub(crate) children: Forest<Node>,
}

#[derive(Debug)]
pub(crate) struct Forest<Node>(pub(crate) Vec<Tree<Node>>);

impl<Node> Forest<Node>
where
    Node: crate::tree::Node + Display,
    Node::Id: Hash + Eq + Copy + Debug,
{
    pub(crate) fn empty() -> Self {
        Forest(Vec::new())
    }

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

    pub(crate) fn sort_by<F>(&mut self, compare: &F)
    where
        F: Fn(&Node, &Node) -> Ordering,
    {
        self.0.sort_by(|a, b| compare(&a.node, &b.node));
        for tree in self.0.iter_mut() {
            tree.children.sort_by(compare);
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

    pub(crate) fn filter<F>(&mut self, filter: F)
    where
        F: Fn(&Node) -> bool,
    {
        self.filter_helper(&filter, false);
    }

    fn filter_helper<F>(&mut self, filter: &F, parent_included: bool) -> bool
    where
        F: Fn(&Node) -> bool,
    {
        let mut any_child_included = false;
        let mut old = Forest(Vec::new());
        std::mem::swap(self, &mut old);
        for mut tree in old.0.into_iter() {
            if parent_included || filter(&tree.node) {
                tree.children.filter_helper(filter, true);
                self.0.push(tree);
                any_child_included = true
            } else if tree.children.filter_helper(filter, false) {
                self.0.push(tree);
                any_child_included = true;
            }
        }
        any_child_included
    }

    pub(crate) fn render_forest_prefixes(&self) -> Vec<(String, &Node)> {
        let mut acc = Vec::new();
        self.render_forest_prefixes_helper(true, &mut Vec::new(), &mut acc);
        acc
    }

    fn render_forest_prefixes_helper<'a>(
        &'a self,
        is_root: bool,
        prefixes: &mut Vec<&str>,
        acc: &mut Vec<(String, &'a Node)>,
    ) {
        for (i, child) in self.0.iter().enumerate() {
            let is_last = i == self.0.len() - 1;
            let mut line = String::new();
            for prefix in prefixes.iter() {
                line += prefix;
            }
            if !is_root {
                line += if is_last { "└─" } else { "├─" };
                let has_children = !child.children.0.is_empty();
                line += if has_children { "┬ " } else { "─ " };
            }
            acc.push((line, &child.node));
            if !(is_root) {
                prefixes.push(if is_last { "  " } else { "│ " });
            }
            child
                .children
                .render_forest_prefixes_helper(false, prefixes, acc);
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
        Node::Id: Eq + Copy + Hash + Debug,
    {
        fn test_format(&self) -> String {
            let table: Vec<String> = self
                .render_forest_prefixes()
                .into_iter()
                .map(|x| format!("{}{}", x.0, x.1))
                .collect();
            format!("{}\n", table.join("\n"))
        }
    }

    #[derive(Debug)]
    struct TestNode {
        id: usize,
        parent: Option<usize>,
    }

    impl Display for TestNode {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", crate::utils::test::render_number(self.id))
        }
    }

    impl Node for TestNode {
        type Id = usize;

        fn id(&self) -> usize {
            self.id
        }

        fn parent(&self) -> Option<usize> {
            self.parent
        }

        fn accumulate_from(&mut self, _other: &Self) {}
    }

    impl TestNode {
        fn new(id: usize, parent: Option<usize>) -> TestNode {
            TestNode { id, parent }
        }
    }

    #[test]
    fn a_single_node_tree() {
        let tree = Forest::new_forest(vec![TestNode::new(1, None)].into_iter());
        assert_eq!(
            tree.test_format(),
            "
                one
            "
            .unindent()
        );
    }

    #[test]
    fn b_child() {
        let tree =
            Forest::new_forest(vec![TestNode::new(1, None), TestNode::new(2, Some(1))].into_iter());
        assert_eq!(
            tree.test_format(),
            "
                one
                └── two
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
            tree.test_format(),
            "
                one
                ├── two
                ├── three
                └── four
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
            tree.test_format(),
            "
                one
                └─┬ two
                  └── three
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
            tree.test_format(),
            "
                one
                ├─┬ two
                │ └── three
                └── four
            "
            .unindent()
        );
    }

    #[test]
    fn f_multiple_roots() {
        let tree =
            Forest::new_forest(vec![TestNode::new(1, None), TestNode::new(2, None)].into_iter());
        assert_eq!(
            tree.test_format(),
            "
                one
                two
            "
            .unindent()
        );
    }

    #[test]
    fn g_allows_sorting_roots_by_cmp() {
        let mut tree =
            Forest::new_forest(vec![TestNode::new(1, None), TestNode::new(2, None)].into_iter());
        tree.sort_by(&|a, b| b.id.cmp(&a.id));
        assert_eq!(
            tree.test_format(),
            "
                two
                one
            "
            .unindent()
        );
    }

    mod h_filtering {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn a_filters_nodes() {
            let mut tree = Forest::new_forest(
                vec![TestNode::new(1, None), TestNode::new(2, None)].into_iter(),
            );
            tree.filter(|node| node.id == 2);
            assert_eq!(
                tree.test_format(),
                "
                    two
                "
                .unindent()
            );
        }

        #[test]
        fn b_shows_children_of_included_nodes() {
            let mut tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, None),
                ]
                .into_iter(),
            );
            tree.filter(|node| node.id == 1);
            assert_eq!(
                tree.test_format(),
                "
                    one
                    └── two
                "
                .unindent()
            );
        }

        #[test]
        fn c_shows_parents_of_included_nodes() {
            let mut tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, None),
                ]
                .into_iter(),
            );

            tree.filter(|node| node.id == 2);
            assert_eq!(
                tree.test_format(),
                "
                    one
                    └── two
                "
                .unindent()
            );
        }

        #[test]
        fn d_shows_transitive_parents() {
            let mut tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                ]
                .into_iter(),
            );
            tree.filter(|node| node.id == 3);
            assert_eq!(
                tree.test_format(),
                "
                    one
                    └─┬ two
                      └── three
                "
                .unindent()
            );
        }

        #[test]
        fn e_bigger() {
            let mut tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                    TestNode::new(4, None),
                ]
                .into_iter(),
            );
            tree.filter(|node| node.id == 2);
            assert_eq!(
                tree.test_format(),
                "
                    one
                    └─┬ two
                      └── three
                "
                .unindent()
            );
        }

        #[test]
        fn f_no_unconnected_lines() {
            let mut tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None),
                    TestNode::new(2, Some(1)),
                    TestNode::new(3, Some(2)),
                    TestNode::new(4, Some(1)),
                ]
                .into_iter(),
            );
            tree.filter(|node| node.id == 2);
            assert_eq!(
                tree.test_format(),
                "
                    one
                    └─┬ two
                      └── three
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

            fn parent(&self) -> Option<u8> {
                self.parent
            }

            fn accumulate_from(&mut self, other: &Self) {
                self.to_accumulate += other.to_accumulate;
            }
        }

        #[test]
        fn a_can_compute_accumulated_values_of_children() {
            let mut tree = Forest::new_forest(
                vec![TestNode::new(1, None, 2), TestNode::new(2, Some(1), 3)].into_iter(),
            );
            tree.filter(|node| node.id == 2);
            assert_eq!(
                tree.test_format(),
                "
                    5
                    └── 3
                "
                .unindent()
            );
        }

        #[test]
        fn b_accumulates_from_grandchildren() {
            let mut tree = Forest::new_forest(
                vec![
                    TestNode::new(1, None, 2),
                    TestNode::new(2, Some(1), 3),
                    TestNode::new(3, Some(2), 8),
                ]
                .into_iter(),
            );
            tree.filter(|node| node.id == 2);
            assert_eq!(
                tree.test_format(),
                "
                    13
                    └─┬ 11
                      └── 8
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
                tree.test_format(),
                "
                    12
                    ├─┬ 5
                    │ └── 4
                    ├─┬ 4
                    │ └── 2
                    └─┬ 3
                      └── 0
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
            eprintln!("{}", tree.test_format());
            assert_eq!(
                tree.iter().map(Node::id).collect::<Vec<usize>>(),
                vec![1, 2, 3, 4]
            );
        }
    }
}
