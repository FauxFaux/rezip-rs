use std;
use std::fmt;

use cast::u16;
use cast::usize;
use failure::ensure;
use failure::Error;
use itertools::Itertools;

use crate::bit::BitSource;
use crate::bit::BitVec;

pub struct CodeTree {
    left: Node,
    right: Node,
}

impl CodeTree {
    pub fn new(canonical_code_lengths: &[u8]) -> Result<Self, Error> {
        ensure!(canonical_code_lengths.len() >= 2, "too few lengths");

        ensure!(
            canonical_code_lengths.len() <= usize(std::u32::MAX),
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
                        .map(|(pos, _)| Node::Leaf(u16(pos).unwrap())),
                );
            }

            // pair up old nodes into internal nodes in the new tree
            new_nodes.extend(
                nodes
                    .into_iter()
                    .tuples()
                    .map(|(first, second)| Node::Internal(Box::new(first), Box::new(second))),
            );

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

    pub fn decode_symbol<B: BitSource>(&self, reader: &mut B) -> Result<u16, Error> {
        decode_symbol_impl(reader, &self.left, &self.right)
    }

    pub fn invert(&self) -> Vec<Option<BitVec>> {
        let mut into = vec![None; 288];

        store_code(&mut into, plus_bit(&BitVec::new(), false), &self.left);
        store_code(&mut into, plus_bit(&BitVec::new(), true), &self.right);

        into
    }
}

fn decode_symbol_impl<B: BitSource>(
    reader: &mut B,
    left: &Node,
    right: &Node,
) -> Result<u16, Error> {
    use self::Node::*;

    match *if reader.read_bit()? { right } else { left } {
        Leaf(sym) => Ok(sym),
        Internal(ref new_left, ref new_right) => decode_symbol_impl(reader, new_left, new_right),
    }
}

enum Node {
    Leaf(u16),
    Internal(Box<Node>, Box<Node>),
}

impl fmt::Debug for CodeTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt(f, "0", &self.left)?;
        fmt(f, "1", &self.right)
    }
}

fn fmt(into: &mut fmt::Formatter, prefix: &str, node: &Node) -> fmt::Result {
    match *node {
        Node::Leaf(sym) => {
            write!(into, "{} => ", prefix)?;
            match sym {
                0...255 => write!(into, "0x{:02x} {:?}\n", sym, sym as u8 as char),
                256 => write!(into, "EoS\n"),
                other => write!(into, "d:{}\n", other - 256),
            }
        }
        Node::Internal(ref left, ref right) => {
            fmt(into, &format!("{}0", prefix), left)?;
            fmt(into, &format!("{}1", prefix), right)
        }
    }
}

fn store_code(into: &mut Vec<Option<BitVec>>, prefix: BitVec, node: &Node) {
    match *node {
        Node::Leaf(sym) => {
            assert!(into[usize(sym)].is_none(), "duplicate code in tree");
            into[usize(sym)] = Some(prefix);
        }
        Node::Internal(ref left, ref right) => {
            store_code(into, plus_bit(&prefix, false), left);
            store_code(into, plus_bit(&prefix, true), right);
        }
    }
}

fn plus_bit(into: &BitVec, val: bool) -> BitVec {
    let mut copy = into.clone();
    copy.push(val);
    copy
}
