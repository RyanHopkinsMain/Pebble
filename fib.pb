// Naive recursion vs. memoization with a map.

fn fib(n) {
  if n < 2 { return n; }
  return fib(n - 1) + fib(n - 2);
}

let memo = {};
fn fast_fib(n) {
  if n < 2 { return n; }
  let key = str(n);
  if memo[key] != nil { return memo[key]; }
  let result = fast_fib(n - 1) + fast_fib(n - 2);
  memo[key] = result;
  return result;
}

let start = clock();
print("fib(24) =", fib(24), "(naive, " + str(floor((clock() - start) * 1000)) + " ms)");
print("fib(78) =", fast_fib(78), "(memoized)");
