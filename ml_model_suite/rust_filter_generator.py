#!/usr/bin/env python3
import os
import joblib
import json
import pandas as pd
from sklearn.tree import DecisionTreeClassifier, _tree

def generate_rust_code(data_path="/home/smhvz/Desktop/cycle-orc/ml_model_suite/data/dataset.csv", output_file="/home/smhvz/Desktop/cycle-orc/ml_model_suite/generated/ml_filter.rs"):
    os.makedirs(os.path.dirname(output_file), exist_ok=True)
    
    df = pd.read_csv(data_path)
    feature_cols = [
        'trend_100b_pct', 'trend_50b_pct', 'trend_20b_pct', 'stoch_pos_pct',
        'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
        'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
        'last_bar_is_bullish'
    ]

    X = df[feature_cols]
    y = df['target']

    dt = DecisionTreeClassifier(max_depth=4, min_samples_leaf=15, random_state=42, class_weight='balanced')
    dt.fit(X, y)

    tree_ = dt.tree_
    feature_names = [feature_cols[i] if i != _tree.TREE_UNDEFINED else "undefined!" for i in tree_.feature]

    rust_lines = []
    rust_lines.append("// Auto-generated ML Filter Rule Engine in Pure Rust")
    rust_lines.append("// Compiled for microsecond execution in Cycle Orc C-ABI Plugins")
    rust_lines.append("")
    rust_lines.append("#[derive(Debug, Clone, Copy)]")
    rust_lines.append("pub struct MLFeatures {")
    for f in feature_cols:
        rust_lines.append(f"    pub {f}: f64,")
    rust_lines.append("}")
    rust_lines.append("")
    rust_lines.append("pub fn evaluate_ml_filter(f: &MLFeatures) -> bool {")

    def recurse(node, depth):
        indent = "    " * (depth + 1)
        if tree_.feature[node] != _tree.TREE_UNDEFINED:
            name = feature_names[node]
            threshold = tree_.threshold[node]
            rust_lines.append(f"{indent}if f.{name} <= {threshold:.4f} {{")
            recurse(tree_.children_left[node], depth + 1)
            rust_lines.append(f"{indent}}} else {{")
            recurse(tree_.children_right[node], depth + 1)
            rust_lines.append(f"{indent}}}")
        else:
            val = tree_.value[node][0]
            total = val.sum()
            win_ratio = val[1] / total if total > 0 else 0.0
            should_trade = "true" if win_ratio >= 0.50 else "false"
            rust_lines.append(f"{indent}// Leaf node: WIN prob = {win_ratio*100:.2f}% ({val[1]}/{total})")
            rust_lines.append(f"{indent}{should_trade}")

    recurse(0, 0)
    rust_lines.append("}")

    rust_code = "\n".join(rust_lines)
    with open(output_file, "w") as out:
        out.write(rust_code)

    print(f"✅ Generated Rust C-ABI ML Filter code: {output_file}")
    return rust_code

if __name__ == "__main__":
    generate_rust_code()
