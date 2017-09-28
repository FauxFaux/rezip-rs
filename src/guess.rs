use Code;

pub fn guess_huffman(codes: &[Code]) {
    println!("{:?}", max_distance(codes))
}

fn max_distance(codes: &[Code]) -> Option<u16> {
    codes
        .iter()
        .flat_map(|code| if let Code::Reference { dist, .. } = *code {
            Some(dist)
        } else {
            None
        })
        .max()
}
