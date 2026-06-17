import urllib.request
url = "https://www.c64-wiki.com/wiki/GCR"
req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
try:
    with urllib.request.urlopen(req) as response:
        html = response.read().decode('utf-8')
        for line in html.split('\n'):
            if 'gap' in line.lower() or 'sync' in line.lower() or 'bytes' in line.lower():
                # print(line.strip())
                pass
except Exception as e:
    print(e)
