use std::cmp;

use errors::*;
use Code;
use WindowSettings;

pub fn max_distance(codes: &[Code]) -> Option<u16> {
    codes
        .iter()
        .flat_map(|code| {
            if let Code::Reference { dist, .. } = *code {
                Some(dist)
            } else {
                None
            }
        })
        .max()
}

/// 1) checks if any code references before the start of this block
/// 2) checks if any code references the exact start of the block
pub fn outside_range_or_hit_zero(codes: &[Code]) -> (bool, bool) {
    let mut pos: u16 = 0;
    let mut hit_zero = false;

    for code in codes {
        if let Code::Reference { dist, .. } = *code {
            if dist == pos {
                hit_zero = true;
            }

            if dist > pos {
                return (true, hit_zero);
            }
        }

        // this can't overflow, as u16::MAX < 32_768 + max emitted_bytes
        pos = pos.checked_add(code.emitted_bytes()).unwrap();

        if pos > 32_768 {
            break;
        }
    }

    return (false, hit_zero);
}

pub fn guess_settings(mut preroll: &[u8], codes: &[Code]) -> Result<WindowSettings> {
    let window_size = max_distance(codes).unwrap();
    let (outside, hits_first_byte) = outside_range_or_hit_zero(codes);

    let config = WindowSettings {
        window_size,
        first_byte_bug: preroll.is_empty() && !hits_first_byte,
    };

    // optimisation
    if !outside {
        preroll = &[];
    }

    return Ok(config);
}

#[cfg(test)]
mod tests {
    use super::max_distance;
    use super::outside_range_or_hit_zero;
    use super::guess_settings;

    use WindowSettings;

    use Code::Literal as L;
    use Code::Reference as R;


    #[test]
    fn range() {
        assert_eq!((false, false), outside_range_or_hit_zero(&[L(5)]));

        assert_eq!(
            (true, false),
            outside_range_or_hit_zero(&[
                R {
                    dist: 1,
                    run_minus_3: 3,
                }
            ],)
        );

        assert_eq!(
            (false, true),
            outside_range_or_hit_zero(&[
                L(5),
                R {
                    dist: 1,
                    run_minus_3: 3,
                }
            ],)
        );

        assert_eq!(
            (false, false),
            outside_range_or_hit_zero(&[
                L(5),
                L(5),
                R {
                    dist: 1,
                    run_minus_3: 3,
                }
            ],)
        );

        // Not an encoding a real tool would generate
        assert_eq!(
            (false, true),
            outside_range_or_hit_zero(&[
                L(5),
                R {
                    dist: 1,
                    run_minus_3: 20,
                },
                R {
                    dist: 15,
                    run_minus_3: 3,
                }
            ],)
        );

        assert_eq!(
            (true, true),
            outside_range_or_hit_zero(&[
                L(5),
                R {
                    dist: 1,
                    run_minus_3: 4,
                },
                R {
                    dist: 15,
                    run_minus_3: 3,
                }
            ],)
        );
    }
    #[test]
    fn guess_first_byte_bug() {
        assert_eq!(
            WindowSettings {
                window_size: 1,
                first_byte_bug: true,
            },
            guess_settings(
                &[],
                &[
                    L(5),
                    L(5),
                    R {
                        dist: 1,
                        run_minus_3: 5,
                    }
                ],
            ).unwrap()
        );
    }
}
