import random

prefixes = [
    "snax", "neo", "taz", "byali", "pasha", "olof", "krimz", "jw", "flusha", "pronax",
    "f0rest", "get_right", "friberg", "xizt", "fifflaren", "allu", "maikelele", "threat",
    "karrigan", "dev1ce", "dupreeh", "xyp9x", "gla1ve", "magisk", "cajunb", "aizy", "kjaerbye",
    "valde", "konfig", "msl", "rubino", "tenzki", "cadian", "snappi", "jugi", "niko",
    "rain", "guardian", "seized", "flamie", "edward", "zeus", "s1mple", "electronic", "boombl4",
    "perfecto", "b1t", "m0nesy", "hunter", "nexa", "jackz", "amanek", "kennyS", "apex",
    "shox", "nbk", "smithzz", "ex6tenz", "rpk", "zywoo", "misutaaa", "kyojin", "spinx",
    "maden", "snappi", "sjuush", "teses", "cadiaN", "stavn", "jabbi", "tecadiaN", "k0nfig",
    "blamef", "farlig", "es3tag", "hampus", "rez", "plopski", "brollan", "nawwk", "ztr",
    "phzy", "s1n", "xertion", "torszi", "dexter", "frozen", "ropz", "broky", "twistzz",
    "naf", "elige", "stewie2k", "tarik", "rush", "autimatic", "skadoodle", "seangares", "n0thing",
    "shroud", "freakazoid", "pasha", "byali", "snax", "neo", "taz", "michu", "rallen",
    "mouz", "oskar", "sunny", "styko", "chrisj", "ropz", "lmbt", "kassad", "ynk",
    "moses", "sadokist", "henryg", "machine", "pansy", "spunj", "stunna", "smix", "frankie",
    "yekindar", "qikert", "jame", "buster", "sanji", "fl1t", "fame", "n0rb3r7", "chopper",
    "magixx", "degster", "patsi", "w0nderful", "s1ren", "donk", "sh1ro", "nafany", "ax1le",
    "interz", "hobbit", "mou", "dosia", "adren", "fitch", "dimasick", "somedieyoung", "mir",
    "leon", "kyle", "brad", "sam", "alex", "john", "michael", "david", "james", "robert"
]

suffixes = [
    "-", "_", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "x", "z", "y", "q", "w",
    "v", "b", "n", "m", "k", "j", "h", "g", "f", "d", "s", "a", "p", "o", "i", "u", "e",
    "R", "Z", "X", "Y", "Q", "W", "V", "B", "N", "M", "K", "J", "H", "G", "F", "D", "S"
]

leet = {
    'a': '4',
    'e': '3',
    'i': '1',
    'o': '0',
    's': '5',
    't': '7'
}

def cap_random(s):
    return ''.join(c.upper() if random.random() < 0.3 else c.lower() for c in s)

def to_leet(s):
    return ''.join(leet.get(c.lower(), c) if random.random() < 0.2 else c for c in s)

names = set()
while len(names) < 1000:
    base = random.choice(prefixes)
    
    mod_type = random.random()
    if mod_type < 0.2:
        # just capitalize random
        n = cap_random(base)
    elif mod_type < 0.4:
        # leetspeak
        n = to_leet(base)
    elif mod_type < 0.6:
        # add suffix
        n = cap_random(base) + random.choice(suffixes)
    elif mod_type < 0.8:
        # add prefix
        n = random.choice(["i", "x", "v", "o"]) + cap_random(base)
    else:
        # shorten
        n = base[:len(base)//2 + 1].upper() + random.choice(suffixes)
        
    if len(n) > 2 and len(n) <= 12:
        names.add(n)

# Also add the exact ones the user mentioned
exacts = ["NAF", "YEKINDAR", "apEX", "shox", "Magisk", "dupreeh", "gla1ve", "Xyp9x", "cadiaN", "m0NESY", "huNter-", "jks", "blameF", "k0nfig", "REZ", "Brollan", "ropz", "broky", "rain", "karrigan", "frozen", "w0xic", "ISSAA", "NBK-", "Happy", "SmithZz", "Ex6TenZ", "ScreaM", "Nivera"]
for e in exacts:
    names.add(e)

names_list = list(names)
random.shuffle(names_list)
names_list = names_list[:1000]

with open("src/names.rs", "w") as f:
    f.write("pub const BOT_NAMES: &[&str] = &[\n")
    for n in names_list:
        f.write(f'    "{n}",\n')
    f.write("];\n")

