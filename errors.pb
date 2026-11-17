// Runtime errors point at the exact spot and show the call chain.
fn average(xs) {
  return sum(xs) / len(xs);
}

fn report(scores) {
  print("average:", average(scores));
}

report([90, 85, 77]);
report([]);
