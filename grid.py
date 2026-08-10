events = [
(0.0, 1, 0),
(42.0, 1, 1),
(80.5, 0, 0),
(88.0, 0, 1),
(96.5, 0, 0),
(115.5, 1, 0),
(153.5, 1, 1),
(163.5, 1, 0),
(172.5, 1, 1),
(180.5, 1, 0),
(188.0, 1, 1),
(200.5, 0, 0),
(220.0, 0, 1),
(228.5, 0, 0),
(240.0, 1, 0),
(248.5, 1, 1),
(260.5, 0, 0),
(300.5, 1, 0),
(320.5, 0, 0),
(328.5, 1, 1),
(340.5, 0, 0),
]

def get_state(t):
    state = (1,1)
    for e in events:
        if e[0] <= t:
            state = (e[1], e[2])
        else:
            break
    return state

last = None
count = 0
for t in range(40, 360, 2):
    s = get_state(t)
    if s == last:
        count += 1
    else:
        if last is not None:
            print(f"{count*2:4d} us: {last[0]}{last[1]}")
        last = s
        count = 1
print(f"{count*2:4d} us: {last[0]}{last[1]}")
