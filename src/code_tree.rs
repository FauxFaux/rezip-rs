use std;
use std::rc::Rc;

use errors::*;

pub struct CodeTree {
    pub left: Rc<Node>,
    pub right: Rc<Node>,
}


impl CodeTree {
    pub fn new(canonical_code_lengths: &[u32]) -> Result<Self> {
        ensure!(canonical_code_lengths.len() >= 2, "too few lengths");
        ensure!(
            canonical_code_lengths.len() <= std::u32::MAX as usize,
            "too many lengths"
        );

        let mut nodes: Vec<Rc<Node>> = Vec::with_capacity(16);

        let fifteen_to_zero_inclusive = (0..16).rev();
        for i in fifteen_to_zero_inclusive {
            ensure!(nodes.len() % 2 == 0, "not a tree");

            let mut new_nodes: Vec<Rc<Node>> = Vec::with_capacity(16);

            if i > 0 {
                for j in 0..canonical_code_lengths.len() {
                    if i == canonical_code_lengths[j] {
                        new_nodes.push(Rc::new(Node::Leaf(j as u32)));
                    }
                }
            }

            for j in 0..nodes.len() / 2 {
                let j = j * 2;
                new_nodes.push(Rc::new(
                    Node::Internal(Rc::clone(&nodes[j]), Rc::clone(&nodes[j + 1])),
                ));
            }

            nodes = new_nodes;
        }

        ensure!(1 == nodes.len(), "non-canonical code");

        match *nodes[0] {
            Node::Internal(ref left, ref right) => Ok(CodeTree {
                left: left.clone(),
                right: right.clone(),
            }),
            Node::Leaf(_) => bail!("root must be a node"),
        }
    }
}

pub enum Node {
    Leaf(u32),
    Internal(Rc<Node>, Rc<Node>),
}
