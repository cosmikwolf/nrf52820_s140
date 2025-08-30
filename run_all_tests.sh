#!/bin/bash

# Test runner script for nRF52820 S140 firmware
# Runs all test files and provides a comprehensive report

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Test tracking variables
declare -a PASSED_TESTS=()
declare -a FAILED_TESTS=()
declare -a TEST_WARNINGS=()
declare -a TEST_ERRORS=()
TOTAL_WARNINGS=0
TOTAL_ERRORS=0

# Create logs directory
mkdir -p test_logs

echo -e "${BOLD}=== nRF52820 S140 Firmware Test Suite Runner ===${NC}"
echo -e "${BLUE}Starting test execution at $(date)${NC}"
echo ""

# Function to run a single test
run_test() {
    local test_name="$1"
    local log_file="test_logs/${test_name}.log"
    local error_file="test_logs/${test_name}_stderr.log"
    
    echo -e "${BLUE}${BOLD}>>> Running test: ${test_name}${NC}"
    echo "----------------------------------------"
    
    # Run the test and capture both stdout and stderr separately
    if timeout 300 cargo test --test "$test_name" > "$log_file" 2> "$error_file"; then
        echo -e "${GREEN}✓ PASSED${NC}"
        PASSED_TESTS+=("$test_name")
        
        # Display stdout
        if [ -s "$log_file" ]; then
            echo -e "${BOLD}Test Output:${NC}"
            cat "$log_file"
        fi
        
    else
        echo -e "${RED}✗ FAILED${NC}"
        FAILED_TESTS+=("$test_name")
        
        # Display stdout
        if [ -s "$log_file" ]; then
            echo -e "${BOLD}Test Output:${NC}"
            cat "$log_file"
        fi
        
        # Display stderr
        if [ -s "$error_file" ]; then
            echo -e "${RED}${BOLD}Error Output:${NC}"
            cat "$error_file"
        fi
    fi
    
    # Count warnings and errors from stderr
    if [ -s "$error_file" ]; then
        local warnings=$(grep -c "warning:" "$error_file" 2>/dev/null || echo "0")
        local errors=$(grep -c "error\(\[E[0-9]\+\]\)\|error:" "$error_file" 2>/dev/null || echo "0")
        
        # Ensure warnings and errors are valid numbers
        if ! [[ "$warnings" =~ ^[0-9]+$ ]]; then
            warnings=0
        fi
        if ! [[ "$errors" =~ ^[0-9]+$ ]]; then
            errors=0
        fi
        
        TEST_WARNINGS+=("$test_name:$warnings")
        TEST_ERRORS+=("$test_name:$errors")
        
        TOTAL_WARNINGS=$((TOTAL_WARNINGS + warnings))
        TOTAL_ERRORS=$((TOTAL_ERRORS + errors))
        
        if [ "$warnings" -gt 0 ]; then
            echo -e "${YELLOW}Warnings: $warnings${NC}"
        fi
        if [ "$errors" -gt 0 ]; then
            echo -e "${RED}Errors: $errors${NC}"
        fi
    else
        TEST_WARNINGS+=("$test_name:0")
        TEST_ERRORS+=("$test_name:0")
    fi
    
    echo ""
    echo "========================================"
    echo ""
}

# Find all test files
echo -e "${BLUE}Discovering test files...${NC}"
TEST_FILES=($(find tests -name "*.rs" -not -name "mod.rs" -not -path "*/common/*" | sed 's|tests/||' | sed 's|\.rs$||' | sort))

if [ ${#TEST_FILES[@]} -eq 0 ]; then
    echo -e "${RED}No test files found!${NC}"
    exit 1
fi

echo -e "${BLUE}Found ${#TEST_FILES[@]} test files:${NC}"
for test in "${TEST_FILES[@]}"; do
    echo "  - $test"
done
echo ""

# Run each test
for test_file in "${TEST_FILES[@]}"; do
    run_test "$test_file"
done

# Generate final report
echo -e "${BOLD}=== FINAL TEST REPORT ===${NC}"
echo -e "${BLUE}Execution completed at $(date)${NC}"
echo ""

echo -e "${BOLD}Summary:${NC}"
echo -e "  Total tests run: ${#TEST_FILES[@]}"
echo -e "  ${GREEN}Passed: ${#PASSED_TESTS[@]}${NC}"
echo -e "  ${RED}Failed: ${#FAILED_TESTS[@]}${NC}"
echo -e "  ${YELLOW}Total warnings: $TOTAL_WARNINGS${NC}"
echo -e "  ${RED}Total errors: $TOTAL_ERRORS${NC}"
echo ""

if [ ${#PASSED_TESTS[@]} -gt 0 ]; then
    echo -e "${GREEN}${BOLD}PASSED TESTS:${NC}"
    for test in "${PASSED_TESTS[@]}"; do
        echo -e "  ${GREEN}✓${NC} $test"
    done
    echo ""
fi

if [ ${#FAILED_TESTS[@]} -gt 0 ]; then
    echo -e "${RED}${BOLD}FAILED TESTS:${NC}"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "  ${RED}✗${NC} $test"
    done
    echo ""
fi

# Detailed warning/error breakdown
if [ $TOTAL_WARNINGS -gt 0 ] || [ $TOTAL_ERRORS -gt 0 ]; then
    echo -e "${BOLD}Detailed Warning/Error Breakdown:${NC}"
    echo "Test File                    | Warnings | Errors"
    echo "----------------------------------------|----------|--------"
    
    for entry in "${TEST_WARNINGS[@]}"; do
        test_name=$(echo "$entry" | cut -d: -f1)
        warnings=$(echo "$entry" | cut -d: -f2)
        
        # Find corresponding errors
        errors=0
        for error_entry in "${TEST_ERRORS[@]}"; do
            if [[ "$error_entry" == "$test_name:"* ]]; then
                errors=$(echo "$error_entry" | cut -d: -f2)
                break
            fi
        done
        
        printf "%-30s | %8s | %6s\n" "$test_name" "$warnings" "$errors"
    done
    echo ""
fi

# Success/failure indicators
if [ ${#FAILED_TESTS[@]} -eq 0 ]; then
    echo -e "${GREEN}${BOLD}🎉 ALL TESTS PASSED! 🎉${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}❌ ${#FAILED_TESTS[@]} TEST(S) FAILED ❌${NC}"
    echo -e "${YELLOW}Check the logs in test_logs/ directory for detailed output${NC}"
    exit 1
fi