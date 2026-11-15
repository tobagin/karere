// Exercise the actual Rust-generated scripts with a minimal DOM, without CEF.
const assert = require('node:assert/strict');
const vm = require('node:vm');
const [update, replacement, removal, encodedCss] = process.argv.slice(1);
const css = JSON.parse(encodedCss);

function page(hasHead = true) {
  const styles = [];
  const root = { appendChild: (element) => styles.push(element) };
  const context = vm.createContext({
    document: {
      head: hasHead ? root : null,
      documentElement: root,
      getElementById: (id) => styles.find((element) => element.id === id),
      createElement(tag) {
        assert.equal(tag, 'style');
        return {
          remove() {
            styles.splice(styles.indexOf(this), 1);
          },
        };
      },
    },
  });
  return { styles, run: (script) => vm.runInContext(script, context) };
}

// Separate pages model independent foreground/background account documents.
for (const account of [page(), page(), page(false)]) {
  account.run(removal); // Missing element is harmless.
  account.run(update);
  assert.equal(account.styles.length, 1);
  assert.equal(account.styles[0].id, 'karere-user-css');
  assert.equal(account.styles[0].textContent, css);
  const element = account.styles[0];
  account.run(update);
  account.run(replacement);
  assert.equal(account.styles.length, 1);
  assert.equal(account.styles[0], element); // Updates reuse the same element.
  assert.equal(element.textContent, 'body { color: blue; }');
  account.run(removal);
  account.run(removal);
  assert.equal(account.styles.length, 0);
  account.run(update); // Recreation after deletion works.
  assert.equal(account.styles.length, 1);
  assert.equal(account.styles[0].textContent, css);
}
