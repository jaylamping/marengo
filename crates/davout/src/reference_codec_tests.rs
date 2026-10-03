#![allow(clippy::expect_used)]
use super::*;
use serde::Deserialize;

#[test]
fn scalar_tags_keep_exact_signed_zero_subnormal_and_integer_widths() {
    for bits in [
        0u64,
        1,
        0x8000_0000_0000_0000,
        0x0010_0000_0000_0000,
        0x3ff0_0000_0000_0001,
        0x7fef_ffff_ffff_ffff,
    ] {
        let expected = [MAGIC.as_slice(), &[12], &bits.to_le_bytes()].concat();
        assert_eq!(
            encode(&f64::from_bits(bits)).expect("f64 exact encoding"),
            expected
        );
        assert_eq!(
            decode::<f64>(&expected)
                .expect("f64 exact decode")
                .to_bits(),
            bits
        );
    }
    for bits in [0u32, 1, 0x8000_0000, 0x0080_0000, 0x3f80_0001, 0x7f7f_ffff] {
        let expected = [MAGIC.as_slice(), &[11], &bits.to_le_bytes()].concat();
        assert_eq!(
            encode(&f32::from_bits(bits)).expect("f32 exact encoding"),
            expected
        );
        assert_eq!(
            decode::<f32>(&expected)
                .expect("f32 exact decode")
                .to_bits(),
            bits
        );
    }
    assert_eq!(
        encode(&u64::MAX).expect("unsigned job width"),
        [MAGIC.as_slice(), &[5], &[255; 8]].concat()
    );
    assert_eq!(
        decode::<u64>(&encode(&u64::MAX).expect("u64")).expect("full u64"),
        u64::MAX
    );
    assert_eq!(
        decode::<i8>(&encode(&-1i8).expect("signed direction")).expect("i8"),
        -1
    );
    let narrow = encode(&1u8).expect("narrow tag");
    assert!(
        decode::<u64>(&narrow).is_err(),
        "canonical reencoding refuses width coercion"
    );
}

#[test]
fn canonical_map_order_and_full_input_validation_are_enforced() {
    let mut a = std::collections::HashMap::<String, u8>::new();
    a.insert("z".to_string(), 2u8);
    a.insert("a".to_string(), 1u8);
    let mut b = std::collections::HashMap::<String, u8>::new();
    b.insert("a".to_string(), 1u8);
    b.insert("z".to_string(), 2u8);
    let expected = [
        MAGIC.as_slice(),
        &[
            17, 2, 0, 0, 0, 14, 1, 0, 0, 0, b'a', 2, 1, 14, 1, 0, 0, 0, b'z', 2, 2,
        ],
    ]
    .concat();
    assert_eq!(encode(&a).expect("canonical map"), expected);
    assert_eq!(encode(&b).expect("different input iteration"), expected);
    for length in 0..expected.len() {
        assert!(decode::<std::collections::HashMap<String, u8>>(&expected[..length]).is_err());
    }
    let mut duplicate = expected.clone();
    duplicate[26] = b'a';
    assert!(decode::<std::collections::HashMap<String, u8>>(&duplicate).is_err());
    let mut trailing = expected;
    trailing.push(0);
    assert!(decode::<std::collections::HashMap<String, u8>>(&trailing).is_err());
}

#[test]
fn unsupported_nonfinite_and_bounded_data_fail_before_truncation() {
    for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(encode(&value).is_err());
    }
    let infinity = [
        MAGIC.as_slice(),
        &[12],
        &f64::INFINITY.to_bits().to_le_bytes(),
    ]
    .concat();
    assert!(decode::<f64>(&infinity).is_err());
    assert!(encode(&"x".repeat(BODY_CAPACITY)).is_err());
    assert!(encode(&vec![0u8; COLLECTION_CAPACITY + 1]).is_err());
    let excessive = [MAGIC.as_slice(), &[16], &u32::MAX.to_le_bytes()].concat();
    assert!(decode::<Vec<u8>>(&excessive).is_err());
    #[derive(Serialize, Deserialize)]
    enum Unsupported {
        Payload(u8),
    }
    assert!(encode(&Unsupported::Payload(1)).is_err());
    #[derive(Serialize, Deserialize)]
    struct Nested {
        child: Option<Box<Nested>>,
    }
    let mut value = Nested { child: None };
    for _ in 0..DEPTH_CAPACITY {
        value = Nested {
            child: Some(Box::new(value)),
        };
    }
    assert!(encode(&value).is_err());
}
