// 30-autocorrect.js — optional auto-correct of common misspellings in editable
// fields, gated on window.__karereAutoCorrect.
//
// Chromium exposes no JS spellcheck/autocorrect API, so we use a small built-in
// typos map. On a word boundary the just-typed word is looked up; unambiguous
// matches are replaced in place, unknown words left untouched.
//
// Flag defaults false, (re)seeded from the `enable-auto-correct` GSettings key
// by handlers/load.rs::apply_autocorrect_from_settings on load + on change.
(function () {
  "use strict";

  if (window.__karereAutoCorrectInstalled) {
    return;
  }
  window.__karereAutoCorrectInstalled = true;
  if (typeof window.__karereAutoCorrect === "undefined") {
    window.__karereAutoCorrect = false;
  }

  // Lowercase typo -> correction. Conservative: only unambiguous fixes.
  var TYPOS = {
    teh: "the",
    adn: "and",
    recieve: "receive",
    recieved: "received",
    seperate: "separate",
    definately: "definitely",
    occured: "occurred",
    untill: "until",
    wich: "which",
    thier: "their",
    becuase: "because",
    alot: "a lot",
    tommorow: "tomorrow",
    wieght: "weight",
    freind: "friend",
    goverment: "government",
    accross: "across",
    agian: "again",
    beleive: "believe",
    enviroment: "environment",
    neccessary: "necessary",
    occassion: "occasion",
    publically: "publicly",
    wether: "whether",
    writting: "writing",
  };

  var BOUNDARY = /[\s.,!?;:]/;

  // Return the corrected word preserving the original capitalization, or null
  // when there is no confident suggestion.
  function correct(word) {
    if (!word) {
      return null;
    }
    var lower = word.toLowerCase();
    var fix = TYPOS[lower];
    if (!fix || fix === lower) {
      return null;
    }
    if (word === lower) {
      return fix; // all lowercase
    }
    if (word === word.toUpperCase()) {
      return fix.toUpperCase(); // ALL CAPS
    }
    if (word.charAt(0) === word.charAt(0).toUpperCase()) {
      return fix.charAt(0).toUpperCase() + fix.slice(1); // Capitalized
    }
    return fix;
  }

  // <input>/<textarea>: operate on .value around the caret.
  function handleInputElement(el) {
    var pos = el.selectionStart;
    if (pos == null) {
      return;
    }
    var value = el.value;
    var boundaryIdx = pos - 1;
    if (boundaryIdx < 0 || !BOUNDARY.test(value.charAt(boundaryIdx))) {
      return;
    }
    var wordEnd = boundaryIdx;
    var wordStart = wordEnd;
    while (wordStart > 0 && !BOUNDARY.test(value.charAt(wordStart - 1))) {
      wordStart--;
    }
    var word = value.slice(wordStart, wordEnd);
    var fix = correct(word);
    if (!fix) {
      return;
    }
    el.value = value.slice(0, wordStart) + fix + value.slice(wordEnd);
    var delta = fix.length - word.length;
    el.selectionStart = el.selectionEnd = pos + delta;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }

  // contenteditable (WhatsApp composer): replace via execCommand so beforeinput/
  // input fire — keeps React in sync and restores the caret automatically.
  function handleContentEditable() {
    var sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || !sel.isCollapsed) {
      return;
    }
    var range = sel.getRangeAt(0);
    var node = range.startContainer;
    if (node.nodeType !== Node.TEXT_NODE) {
      return;
    }
    var offset = range.startOffset;
    var text = node.nodeValue || "";
    var boundaryIdx = offset - 1;
    if (boundaryIdx < 0 || !BOUNDARY.test(text.charAt(boundaryIdx))) {
      return;
    }
    var wordEnd = boundaryIdx;
    var wordStart = wordEnd;
    while (wordStart > 0 && !BOUNDARY.test(text.charAt(wordStart - 1))) {
      wordStart--;
    }
    var word = text.slice(wordStart, wordEnd);
    var fix = correct(word);
    if (!fix) {
      return;
    }
    var wordRange = document.createRange();
    wordRange.setStart(node, wordStart);
    wordRange.setEnd(node, wordEnd);
    sel.removeAllRanges();
    sel.addRange(wordRange);
    var ok = document.execCommand && document.execCommand("insertText", false, fix);
    if (!ok) {
      // Restore the caret unchanged if the editor refused the edit.
      sel.removeAllRanges();
      sel.addRange(range);
    }
  }

  var applying = false;

  function onInput(e) {
    if (applying || !window.__karereAutoCorrect) {
      return;
    }
    var t = e.target;
    if (!t) {
      return;
    }
    applying = true;
    try {
      if (t.tagName === "INPUT" || t.tagName === "TEXTAREA") {
        handleInputElement(t);
      } else if (t.isContentEditable) {
        handleContentEditable();
      }
    } catch (_err) {
      /* never break the page's own input handling */
    }
    applying = false;
  }

  try {
    document.addEventListener("input", onInput, true);
  } catch (_e) {
    /* addEventListener unavailable — nothing to do */
  }
})();
