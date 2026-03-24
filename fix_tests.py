#!/usr/bin/env python3
import re
import sys

def main():
    filename = "tests/synapsis_database_tests.rs"
    with open(filename, 'r') as f:
        content = f.read()
    
    # Track if we're inside the database_tests module
    lines = content.split('\n')
    inside_module = False
    output_lines = []
    
    i = 0
    while i < len(lines):
        line = lines[i]
        
        # Check for module start
        if line.strip() == "mod database_tests {":
            inside_module = True
            output_lines.append(line)
            i += 1
            continue
        
        # Check for module end (assuming it's the last '}' at top level)
        if inside_module and line.strip() == "}" and i > 0 and lines[i-1].strip() == "":
            # This might be the module end, but we need to be careful
            # For simplicity, we'll assume it's the module end
            inside_module = False
            output_lines.append(line)
            i += 1
            continue
        
        if inside_module:
            # Replace cleanup_test_dir(); followed by let db = test_db();
            if line.strip() == "cleanup_test_dir();" and i+1 < len(lines):
                next_line = lines[i+1]
                if next_line.strip().startswith("let db = test_db();"):
                    # Replace with new pattern
                    indent = len(line) - len(line.lstrip())
                    output_lines.append(" " * indent + "let ctx = TestContext::new();")
                    output_lines.append(" " * indent + "let db = ctx.db();")
                    i += 2  # Skip both lines
                    continue
            
            # Remove cleanup_test_dir(); that appears right before a closing brace
            if line.strip() == "cleanup_test_dir();" and i+1 < len(lines):
                next_line = lines[i+1]
                if next_line.strip() == "}":
                    # Skip this line (don't add it to output)
                    i += 1
                    continue
            
            # Remove cleanup_test_dir(); that appears after assertions before closing brace
            # Look for pattern: line with cleanup, next line has only }
            if line.strip() == "cleanup_test_dir();":
                # Check if next non-empty line is }
                j = i + 1
                while j < len(lines) and lines[j].strip() == "":
                    j += 1
                if j < len(lines) and lines[j].strip() == "}":
                    # Skip this cleanup line
                    i += 1
                    continue
        
        # Default: keep the line
        output_lines.append(line)
        i += 1
    
    new_content = '\n'.join(output_lines)
    
    # Write back
    with open(filename, 'w') as f:
        f.write(new_content)
    
    print(f"Processed {filename}")

if __name__ == "__main__":
    main()