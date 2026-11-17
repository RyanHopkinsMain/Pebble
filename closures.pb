// Functions are values, and they remember the scope they were made in.

fn make_counter() {
  let count = 0;
  return fn() {
    count = count + 1;
    return count;
  };
}

let a = make_counter();
let b = make_counter();
a(); a();
print("a:", a(), " b:", b());   // each counter has its own `count`

fn compose(f, g) { return fn(x) { return f(g(x)); }; }
let inc = fn(x) { return x + 1; };
let double = fn(x) { return x * 2; };
print("double then inc:", compose(inc, double)(5));

// Higher-order functions from the prelude (written in Pebble itself).
let squares = map(range(1, 11), fn(n) { return n * n; });
print("squares:", squares);
print("even squares:", filter(squares, fn(n) { return n % 2 == 0; }));
print("sum:", sum(squares));
