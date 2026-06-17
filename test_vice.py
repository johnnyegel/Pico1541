import urllib.request
url = "https://raw.githubusercontent.com/VICE-Team/svn-mirror/master/vice/src/diskimage/diskimage.c"
req = urllib.request.Request(url)
try:
    with urllib.request.urlopen(req) as response:
        content = response.read().decode('utf-8', errors='ignore')
        # We need to find the D64 to G64 conversion or interleave logic
        lines = content.split('\n')
        for i, line in enumerate(lines):
            if 'interleave' in line.lower() or 'format' in line.lower():
                pass
except Exception as e:
    print(e)
