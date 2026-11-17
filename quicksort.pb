fn quicksort(xs) {
  if len(xs) <= 1 { return xs; }
  let pivot = xs[0];
  let rest = [];
  for i in range(1, len(xs)) { push(rest, xs[i]); }
  let smaller = filter(rest, fn(x) { return x < pivot; });
  let larger = filter(rest, fn(x) { return x >= pivot; });
  return quicksort(smaller) + [pivot] + quicksort(larger);
}

// A tiny linear congruential generator, so the output is reproducible.
let seed = 42;
fn rand(n) {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed % n;
}

let data = map(range(12), fn(_) { return rand(100); });
print("before:", data);
let sorted = quicksort(data);
print("after: ", sorted);
assert(sorted == sort(data), "quicksort agrees with the builtin sort");
