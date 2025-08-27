#![no_std]
#![no_main]

mod common;

#[defmt_test::tests]
mod tests {
    use crate::common::*;
    use defmt::{assert, assert_eq};

    #[test]
    fn test_basic_arithmetic() {
        log_test_start("basic_arithmetic");
        assert_eq!(2 + 2, 4);
        assert_eq!(10 - 5, 5);
        assert_eq!(3 * 4, 12);
        assert_eq!(8 / 2, 4);
        log_test_pass("basic_arithmetic");
    }

    #[test]
    fn test_boolean_logic() {
        log_test_start("boolean_logic");
        assert!(true);
        assert!(!false);
        assert_eq!(true && true, true);
        assert_eq!(true || false, true);
        assert_eq!(false && true, false);
        log_test_pass("boolean_logic");
    }

    #[test]
    fn test_array_operations() {
        log_test_start("array_operations");
        let arr = [1, 2, 3, 4, 5];

        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0], 1);
        assert_eq!(arr[4], 5);

        let mut sum = 0;
        for &val in &arr {
            sum += val;
        }
        assert_eq!(sum, 15);
        log_test_pass("array_operations");
    }

    #[test]
    fn test_helper_functions() {
        log_test_start("helper_functions");
        
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
        
        log_test_pass("helper_functions");
    }
}
