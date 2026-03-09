#!/usr/bin/env python3
"""
Parse Cargo.toml to extract binary configurations for the Makefile
"""
import re
import sys

def parse_cargo_toml(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Find all [[bin]] sections
    sections = re.findall(r'\[\[bin\]\]\s*\n(.*?)(?=\n\s*\[|\Z)', content, re.DOTALL)

    results = []
    for section in sections:
        name_match = re.search(r'name\s*=\s*["\']([^"\']*)["\']', section)
        path_match = re.search(r'path\s*=\s*["\']([^"\']*)["\']', section)
        features_match = re.search(r'required-features\s*=\s*\[(.*?)\]', section, re.DOTALL)
        
        if name_match and path_match:
            name = name_match.group(1)
            path = path_match.group(1).split('/')[-1].replace('.rs', '')
            features = ''
            if features_match:
                features_str = features_match.group(1)
                features_list = re.findall(r'["\']([^"\']*)["\']', features_str)
                features = ','.join(features_list)
            
            results.append(f'{path}:{name}:{features}')
    
    return results

if __name__ == '__main__':
    cargo_file = sys.argv[1] if len(sys.argv) > 1 else 'Cargo.toml'
    
    print("# Auto-generated binary mapping")
    for result in parse_cargo_toml(cargo_file):
        print(result)
