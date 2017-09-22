use std;
use std::io::Read;

use itertools::Itertools;

use bit;
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

        let mut nodes: Vec<Node> = Vec::new();

        let fifteen_to_zero_inclusive = (0..16).rev();
        for i in fifteen_to_zero_inclusive {
            ensure!(nodes.len() % 2 == 0, "not a tree");

            let mut new_nodes = Vec::with_capacity(nodes.len() / 2 + canonical_code_lengths.len());

            // add leaves for matching positive lengths
            if i > 0 {
                new_nodes.extend(
                    canonical_code_lengths
                        .iter()
                        .enumerate()
                        .filter(|&(_, val)| i == *val)
                        .map(|(pos, _)| Node::Leaf(pos as u32)),
                );
            }

            // pair up old nodes into internal nodes in the new tree
            new_nodes.extend(nodes.into_iter().tuples().map(|(first, second)| {
                Node::Internal(Box::new(first), Box::new(second))
            }));

            nodes = new_nodes;
        }

        ensure!(1 == nodes.len(), "non-canonical code");

        match nodes.into_iter().next().unwrap() {
            Node::Internal(left, right) => Ok(CodeTree {
                left: *left,
                right: *right,
            }),
            Node::Leaf(_) => panic!("root must be a node"),
        }
    }

    pub fn decode_symbol<R: Read>(&self, reader: &mut bit::BitReader<R>) -> Result<u32> {
        decode_symbol_impl(reader, &self.left, &self.right)
    }
}

fn decode_symbol_impl<R: Read>(
    reader: &mut bit::BitReader<R>,
    left: &Node,
    right: &Node,
) -> Result<u32> {
    use self::Node::*;

    match *if reader.read_bit()? { right } else { left } {
        Leaf(sym) => Ok(sym),
        Internal(ref new_left, ref new_right) => decode_symbol_impl(reader, new_left, new_right),
    }
}

pub enum Node {
    Leaf(u32),
    Internal(Box<Node>, Box<Node>),
}
