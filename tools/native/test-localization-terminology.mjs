import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const clientRoot = resolve(process.env.FFONE_CLIENT_ROOT ??
  resolve(dirname(fileURLToPath(import.meta.url)), '../../../FFOneClient'));

const bundle = (locale) => JSON.parse(readFileSync(
  resolve(clientRoot, `assets/game/localization/${locale}.json`), 'utf8',
)).entries;
const en = bundle('en');
const ru = bundle('ru');
const placeholders = (text) => [...text.matchAll(/\{[^{}]*\}/g)].map(([value]) => value).sort();

test('production EN/RU bundles have identical keys and placeholder multiplicities', () => {
  assert.deepEqual(Object.keys(ru).sort(), Object.keys(en).sort());
  for (const [key, english] of Object.entries(en)) {
    assert.equal(typeof ru[key], 'string', key);
    assert.deepEqual(placeholders(ru[key]), placeholders(english), key);
  }
});

// Approved terminology is independent of the translation being checked. Check every
// occurrence, including both table strings and semantic runtime aliases.
test('standalone terms and names follow the approved Russian glossary', () => {
  const terms = new Map([
    ['Fusion Matter', 'Псевдо-Материя'], ['Taros', 'Таро'],
    ['Unstable Nano', 'Нестабильное Нано'], ['SOOPER', 'СУПЕР'],
    ['Computress', 'Компьютер'], ['Buttercup', 'Пестик'], ['Blossom', 'Цветик'],
    ['Bubbles', 'Пузырёк'], ['Mandark', 'Мэндарк'], ['Belladonna', 'Беладонна'],
    ['Grim', 'Смерть'], ['Courage', 'Кураж'], ['Cheese', 'Чиз'],
    ['Numbuh One', 'Номер Один'], ['Numbuh Two', 'Номер Два'],
    ['Numbuh Three', 'Номер Три'], ['Numbuh Four', 'Номер Четыре'],
    ['Numbuh Five', 'Номер Пять'], ['Stickybeard', 'Сладкая Борода'],
    ['Candy Cove', 'Конфетная Бухта'], ['Eternal Vistas', 'Неизменная Аллея'],
    ['Eternal Meadows', 'Неизменные Луга'], ['City Hall', 'Ратуша'],
    ['Morbucks Towers', 'Башни Дай-Денег'], ['Tech Square', 'Сквер Технологий'],
    ['Upper Catacombs', 'Верхние Катакомбы'], ['Lower Catacombs', 'Нижние Катакомбы'],
    ['AmpFibian', 'АмпФибия'],
  ].map(([english, russian]) => [english.toLowerCase(), russian]));
  const reached = new Set();
  for (const [key, english] of Object.entries(en)) {
    const term = english.trim().toLowerCase();
    if (!terms.has(term)) continue;
    reached.add(term);
    const expected = english === english.toUpperCase()
      ? terms.get(term).toUpperCase() : terms.get(term);
    assert.equal(ru[key], expected, key);
  }
  assert.deepEqual([...reached].sort(), [...terms.keys()].sort(), 'missing production terms');
});

test('tutorial terminology keeps speaker labels, inflection, and dynamic arguments', () => {
  const expected = {
    'tutorial.chat.intro.buttercup': 'ПЕСТИК: Осторожно!!',
    'tutorial.chat.computress_instruction': 'КОМПЬЮТЕР: {instruction}',
    'tutorial.instruction.talk_to_buttercup': 'Поговорите с Пестиком.',
    'tutorial.instruction.warp_into_fusion_lair': 'Нажмите кнопку «Переместиться», чтобы войти в логово Псевдо Пестика.',
    'ui.editor.nano.holo': 'Нестабильное Нано',
  };
  for (const [key, text] of Object.entries(expected)) {
    assert.ok(Object.hasOwn(en, key), key);
    assert.equal(ru[key], text, key);
  }
});

test('item variants use declined character names consistently', () => {
  const expected = new Map([
    ['AmpFibian Helmet', 'Шлем АмпФибии'],
    ['Ultimate Humungousaur Tail', 'Хвост Сверх Гигантозавра'],
    ['Wildmutt Torso', 'Торс Космического Пса'],
    ['Heatblast Boots', 'Сапоги Человека-Огня'],
    ['Upchuck Mouth', 'Рот Блеваки'],
  ]);
  const reached = new Set();
  for (const [key, english] of Object.entries(en)) {
    if (!expected.has(english.trim())) continue;
    reached.add(english.trim());
    assert.equal(ru[key], expected.get(english.trim()), key);
  }
  assert.deepEqual([...reached].sort(), [...expected.keys()].sort());
});
