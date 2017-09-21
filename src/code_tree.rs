use std;

use errors::*;

pub struct CodeTree {
    pub left: Node,
    pub right: Node,
}


impl CodeTree {
    pub fn new(canonical_code_lengths: &[u32]) -> Result<Self> {
        ensure!(canonical_code_lengths.len() >= 2, "too few lengths");
        ensure!(
            canonical_code_lengths.len() <= std::u32::MAX as usize,
            "too many lengths"
        );

        let mut nodes: Vec<Box<Node>> = Vec::with_capacity(16);

        let fifteen_to_zero_inclusive = (0..16).rev();
        for i in fifteen_to_zero_inclusive {
            ensure!(nodes.len() % 2 == 0, "not a tree");

            let mut new_nodes: Vec<Box<Node>> = Vec::with_capacity(16);

            // add leaves for positive lengths
            if i > 0 {
                for j in 0..canonical_code_lengths.len() {
                    if i == canonical_code_lengths[j] {
                        new_nodes.push(Box::new(Node::Leaf(j as u32)));
                    }
                }
            }

            let mut iter = nodes.into_iter();
            loop {
                let first = match iter.next() {
                    Some(x) => x,
                    None => break,
                };

                new_nodes.push(Box::new(Node::Internal(
                    first,
                    iter.next().expect("list is even in length"),
                )));
            }

            nodes = new_nodes;
        }

        ensure!(1 == nodes.len(), "non-canonical code");

        match *nodes.into_iter().next().unwrap() {
            Node::Internal(left, right) => Ok(CodeTree {
                left: *left,
                right: *right,
            }),
            Node::Leaf(_) => panic!("root must be a node"),
        }
    }
}

#[derive(Clone)]
pub enum Node {
    Leaf(u32),
    Internal(Box<Node>, Box<Node>),
}
