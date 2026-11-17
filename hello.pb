// Variables, strings, lists and maps.
let name = "Pebble";
let langs = ["Rust", "Go", "Python"];
let born = {"Rust": 2015, "Go": 2012, "Python": 1991};

print("Hello from " + name + "!");
for lang in langs {
  print(lang, "turned 1.0 in", born[lang]);
}
print("oldest:", sort(map(langs, fn(l) { return born[l]; }))[0]);
