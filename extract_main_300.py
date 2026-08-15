import json

transcript_path = "/home/smhvz/.gemini/antigravity/brain/300dd9eb-7dc6-4741-8a5d-7c282255bcde/.system_generated/logs/transcript_full.jsonl"

found_content = None
with open(transcript_path, 'r') as f:
    for line in f:
        data = json.loads(line)
        if data.get("type") == "VIEW_FILE" and "main.rs" in data.get("content", ""):
            found_content = data.get("content")

if found_content:
    with open("main_from_300.bak", "w") as f:
        f.write(found_content)
    print("Saved to main_from_300.bak")
else:
    print("Not found")
