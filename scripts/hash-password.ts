import { hash } from "@node-rs/argon2";

async function main() {
  const password = process.argv[2];
  if (!password) {
    console.error("Usage: npm run hash-password -- \"a strong password\"");
    process.exitCode = 1;
    return;
  }
  console.log(await hash(password));
}

void main();
