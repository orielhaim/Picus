#!/usr/bin/env bun
const process = require("process");

const BABEL_MAX_NAME_LENGTH = 2;
const BABEL_ALPHABET_LENGTH = 70;
const BABEL_DIRECTORY_OBJECTS = BABEL_ALPHABET_LENGTH ** BABEL_MAX_NAME_LENGTH;

const alphabet = [
  "A", "B", "C", "D", "E", "F", "G", "H",
  "I", "J", "K", "L", "M", "N", "O", "P",
  "Q", "R", "S", "T", "U", "V", "W", "X",
  "Y", "Z", "a", "b", "c", "d", "e", "f",
  "g", "h", "i", "j", "k", "l", "m", "n",
  "o", "p", "q", "r", "s", "t", "u", "v",
  "w", "x", "y", "z", "0", "1", "2", "3",
  "4", "5", "6", "7", "8", "9", "!", " ",
  "&", "(", ")", "-", "_", "+"
];

const RADIX = BigInt(BABEL_ALPHABET_LENGTH);
const BASE = BigInt(BABEL_DIRECTORY_OBJECTS);

const filePath = process.argv[2];
if (!filePath) {
  console.error("No file specified!");
  process.exit(1);
}

const file = Bun.file(filePath);
if (!(await file.exists())) {
  console.error("File not found!");
  process.exit(1);
}

let index = 0n;
let place = 1n;
const reader = file.stream().getReader();
for (;;) {
  const { done, value } = await reader.read();
  if (done) break;
  for (const byte of value) {
    index += BigInt(byte + 1) * place;
    place <<= 8n;
  }
}

let path = "";
while (index) {
  index -= 1n;
  let handle = index % BASE;
  for (let i = 0; i < BABEL_MAX_NAME_LENGTH; i++) {
    path = alphabet[Number(handle % RADIX)] + path;
    handle /= RADIX;
  }
  path = "\\" + path;
  index /= BASE;
}

console.log(path);
