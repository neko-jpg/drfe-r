#!/usr/bin/env python3
"""Remove duplicate code from routing.rs"""

with open('/mnt/c/dev/network test/src/routing.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find and remove lines 480-492 (0-indexed: 479-491)
# Line 480 starts with "}縺ｯ縺昴・..." - detect this broken line
new_lines = []
skip_until = None
for i, line in enumerate(lines):
    line_num = i + 1  # 1-indexed
    
    # Detect the start of duplicate block (line 480)
    if '}縺ｯ縺昴・縺ｾ縺ｾ' in line or (line_num >= 480 and line_num <= 492 and skip_until is None):
        if '}縺ｯ' in line:
            # Replace broken line with just "}"
            new_lines.append('                }\n')
            skip_until = 493  # Skip until line 493
            print(f"Found broken line at {line_num}, skipping until line {skip_until}")
            continue
    
    if skip_until and line_num < skip_until:
        print(f"Skipping line {line_num}: {line[:50]}...")
        continue
    
    new_lines.append(line)

with open('/mnt/c/dev/network test/src/routing.rs', 'w', encoding='utf-8') as f:
    f.writelines(new_lines)

print(f"Done - removed duplicate code. New line count: {len(new_lines)}")
