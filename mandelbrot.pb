// The Mandelbrot set in ASCII: for each point c, iterate z = z^2 + c and see
// how quickly it escapes. Points that never escape are in the set ("@").

let shades = split(" .,:;-=+*#%", "");
let width = 76;
let height = 26;
let max_iter = 80;

for row in range(height) {
  let line = "";
  for col in range(width) {
    let cr = -2.15 + col * 2.75 / width;
    let ci = -1.15 + (row + 0.5) * 2.3 / height;
    let zr = 0;
    let zi = 0;
    let i = 0;
    while i < max_iter and zr * zr + zi * zi < 4 {
      let t = zr * zr - zi * zi + cr;
      zi = 2 * zr * zi + ci;
      zr = t;
      i = i + 1;
    }
    if i == max_iter { line = line + "@"; }
    else {
      let shade = floor(i / 3);
      if shade >= len(shades) { shade = len(shades) - 1; }
      line = line + shades[shade];
    }
  }
  print(line);
}
