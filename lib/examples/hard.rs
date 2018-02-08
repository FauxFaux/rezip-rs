use std::io::Write;

fn main() {
    let chars: Box<[u8]> = (32..128)
        .filter(|&c| (c as char).is_ascii_graphic())
        .collect::<Vec<u8>>()
        .into_boxed_slice();

    let stdout = ::std::io::stdout();
    let mut stdout = stdout.lock();

    let len = chars.len() as u64;

    for i in 0u64.. {
        let mut buf = [0u8; 8];
        let mut j = i;
        let mut k = 0;
        while 0 != j {
            buf[k] = chars[(j % len) as usize];
            k += 1;
            j /= len;
        }
        buf[k] = b'\n';

        stdout.write_all(&buf[..k + 1]).unwrap();
    }
}
