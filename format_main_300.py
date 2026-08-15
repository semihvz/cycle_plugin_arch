import re

with open("main_from_300.bak", "r") as f:
    lines = f.readlines()

out = []
started = False
for line in lines:
    if line.startswith("The following code has been modified to include a line number"):
        started = True
        continue
    if line.startswith("The above content shows the entire"):
        break
    if line.startswith("<truncated"):
        continue
    
    if started:
        # Match pattern "123: content\n"
        match = re.match(r'^\d+:\s?(.*)', line)
        if match:
            out.append(match.group(1) + "\n")
        else:
            out.append(line)

with open("orchestrator/src/main.rs", "w") as f:
    f.write("".join(out))

print("Restored orchestrator/src/main.rs")
