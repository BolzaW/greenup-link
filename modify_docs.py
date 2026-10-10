import re
with open('docs/EVCC_INTEGRATION.md', 'r', encoding='utf-8') as f:
    text = f.read()

text = re.sub(r'# Clean HTTP setter.*?fi"\n', '''# Script setter with JSON body. EVCC replaces ${enable} with true/false
    enable:
      source: script
      cmd: /bin/sh -c "if [ '${enable}' = 'true' ]; then curl -s -X POST -H 'Content-Type: application/json' -d '{\\"action\\":\\"enable\\"}' http://<GREENUP_IP>:3000/api/charge; else curl -s -X POST -H 'Content-Type: application/json' -d '{\\"action\\":\\"disable\\"}' http://<GREENUP_IP>:3000/api/charge; fi"
''', text, flags=re.DOTALL)
with open('docs/EVCC_INTEGRATION.md', 'w', encoding='utf-8') as f:
    f.write(text)
