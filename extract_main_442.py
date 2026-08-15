import json

transcript_path = "/home/smhvz/.gemini/antigravity/brain/09b9f376-ebee-4874-a5a2-a77e3176110a/.system_generated/logs/transcript_full.jsonl"

found_content = None
with open(transcript_path, 'r') as f:
    for line in f:
        data = json.loads(line)
        if data.get("step_index", 0) >= 442:
            break
        if data.get("type") == "VIEW_FILE" and "main.rs" in data.get("content", ""):
            found_content = data.get("content")
        elif data.get("type") == "CODE_ACTION" and "main.rs" in data.get("content", ""):
            # We don't want to just grab code actions, but maybe we can just get the latest view_file before 442
            pass

if found_content:
    with open("main_before_442.bak", "w") as f:
        f.write(found_content)
    print("Saved to main_before_442.bak")
else:
    print("Not found")
