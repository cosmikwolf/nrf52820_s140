#![no_std]
#![no_main]

mod common;

#[defmt_test::tests]
mod tests {
    // Import alloc for Vec usage in property tests
    use alloc::vec::Vec;

    use defmt::{assert, assert_eq};

    use crate::common::*;

    #[init]
    fn init() {
        ensure_heap_initialized();
    }

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(2 + 2, 4);
        assert_eq!(10 - 5, 5);
        assert_eq!(3 * 4, 12);
        assert_eq!(8 / 2, 4);
    }

    #[test]
    fn test_boolean_logic() {
        assert!(true);
        assert!(!false);
        assert_eq!(true && true, true);
        assert_eq!(true || false, true);
        assert_eq!(false && true, false);
    }

    #[test]
    fn test_array_operations() {
        let arr = [1, 2, 3, 4, 5];

        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0], 1);
        assert_eq!(arr[4], 5);

        let mut sum = 0;
        for &val in &arr {
            sum += val;
        }
        assert_eq!(sum, 15);
    }

    #[test]
    fn test_helper_functions() {
        // Test create_test_data helper
        let data = create_test_data(5, 0xAA);
        assert_eq!(data.len(), 5);
        assert_eq!(data[0], 0xAA);
        assert_eq!(data[1], 0xAB);
        assert_eq!(data[4], 0xAE);

        // Test arrays_equal helper
        let arr1 = [1, 2, 3];
        let arr2 = [1, 2, 3];
        let arr3 = [1, 2, 4];
        assert!(arrays_equal(&arr1, &arr2));
        assert!(!arrays_equal(&arr1, &arr3));
    }

    #[test]
    fn test_proptest_arithmetic_properties() {
        // Property: Addition is commutative
        fn addition_commutative(a: u16, b: u16) -> bool {
            let (sum1, overflow1) = a.overflowing_add(b);
            let (sum2, overflow2) = b.overflowing_add(a);
            sum1 == sum2 && overflow1 == overflow2
        }

        // Test with a small sample of values since this runs on hardware
        let test_cases = [
            (0u16, 0u16),
            (1, 0),
            (0, 1),
            (100, 200),
            (u16::MAX, 0),
            (u16::MAX, 1),
            (u16::MAX / 2, u16::MAX / 2),
            (12345, 54321),
        ];

        for (a, b) in &test_cases {
            assert!(addition_commutative(*a, *b));
        }

        // Property: Multiplication by zero always gives zero
        let multiplication_test_values = [0u8, 1, 42, 100, 255];
        for val in &multiplication_test_values {
            assert_eq!(val * 0, 0);
            assert_eq!(0 * val, 0);
        }

        // Property: Array indexing and length consistency
        for len in 1..=10 {
            let test_data = create_test_data(len, 0x42);
            assert_eq!(test_data.len(), len);

            // All elements should be accessible
            for i in 0..len {
                assert!(test_data[i] >= 0x42);
                assert!(test_data[i] <= 0x42 + len as u8);
            }
        }
    }

    #[test]
    fn test_proptest_data_invariants() {
        // Property: create_test_data generates correct patterns
        let pattern_tests = [(5, 0x00), (3, 0xFF), (10, 0x80), (1, 0x42)];

        for (size, pattern) in &pattern_tests {
            let data = create_test_data(*size, *pattern);

            // Check length
            assert_eq!(data.len(), *size);

            // Check pattern progression
            for (i, &value) in data.iter().enumerate() {
                let expected = pattern.wrapping_add(i as u8);
                assert_eq!(value, expected);
            }
        }

        // Property: arrays_equal is reflexive, symmetric, and transitive
        let test_arrays = [
            Vec::from([1u8, 2, 3]),
            Vec::from([255u8]),
            Vec::from([0u8, 100, 200]),
            Vec::from([]),
        ];

        for arr in &test_arrays {
            // Reflexive: array equals itself
            assert!(arrays_equal(arr, arr));

            // Symmetric: if a == b then b == a
            let arr_clone = arr.clone();
            assert!(arrays_equal(arr, &arr_clone));
            assert!(arrays_equal(&arr_clone, arr));
        }

        // Property: Different arrays are not equal
        assert!(!arrays_equal(&[1, 2, 3], &[1, 2, 4]));
        assert!(!arrays_equal(&[1], &[1, 2]));
        assert!(!arrays_equal(&[], &[1]));
    }
}
