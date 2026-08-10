events = []
with open("dump_states.py", "r") as f:
    pass # I already have the output!

# Wait, I will just write a python script to simulate the C64!
code = """
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

# The C64 waits for (1,1) at 42.0 us.
# Let's say it samples at 42 + offset, then every 8 us for 4 samples.
# Then loop overhead is X.
# But AR6 fastloader is exactly known! Let's just sample every 2 us and see!
for t in range(40, 360, 2):
    s = get_state(t)
    print(f"{t:4d} us: {s[0]}{s[1]}")
"""
with open("grid.py", "w") as f:
    f.write(code)
