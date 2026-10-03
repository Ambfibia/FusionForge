"""Build an offline, searchable gallery from the donor texture manifest."""
import collections
import html
import json
import pathlib
import sys
from PIL import Image

root = pathlib.Path(sys.argv[1])
report = json.loads((root / 'manifest.json').read_text(encoding='utf-8'))
thumbs = root / '_preview'
thumbs.mkdir(exist_ok=True)
cards = []
for row in report['textures']:
    src = row['sources'][0]
    path = root / row['file']
    thumb = thumbs / (row['pixels'] + '.png')
    with Image.open(path) as image:
        image.thumbnail((160, 160))
        image.save(thumb)
    sources = sorted({s['source'] for s in row['sources']})
    search = ' '.join(sources + [s['relative'] for s in row['sources']] + [row['category'], row['status'], src['name']])
    candidates = sorted({p for s in row['sources'] for p in s['nativeNameCandidates']})
    note = 'Есть совпадение имени, пиксели отличаются' if candidates else 'Точного совпадения в клиенте нет; новая сущность не доказана'
    cards.append('<a class="card" target="_blank" href="' + html.escape(row['file'], quote=True) +
        '" data-search="' + html.escape(search.lower(), quote=True) + '"><img loading="lazy" src="_preview/' + thumb.name +
        '"><b>' + html.escape(src['name']) + '</b><span>' + html.escape(row['category']) + ' · ' +
        ' × '.join(map(str,row['size'])) + '</span><small>' + html.escape(', '.join(sources)) + '</small><small>' + note + '</small></a>')
document = '''<!doctype html><html lang="ru"><meta charset="utf-8"><title>FusionFall — найденные текстуры</title>
<style>body{font:16px system-ui;background:#131a23;color:#e7edf5;margin:28px}h1{margin-bottom:8px}p{max-width:1000px;color:#bac8d9}input{padding:12px;width:min(700px,90%);font:inherit;border-radius:8px;border:1px solid #5b718c;background:#243247;color:white;position:sticky;top:12px}#grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:14px;margin-top:24px}.card{display:flex;flex-direction:column;gap:7px;color:inherit;text-decoration:none;padding:12px;background:#243247;border-radius:10px;overflow-wrap:anywhere}.card img{height:160px;object-fit:contain;image-rendering:auto;background:repeating-conic-gradient(#56606e 0% 25%,#374151 0% 50%) 50%/18px 18px}.card small{color:#adbed5;font-size:11px}.card span{font-size:13px}#count{padding:12px}</style>
<h1>FusionFall — найденные текстуры</h1><p>Только изображения без точного совпадения с текущими PNG клиента. Сравнены размеры и RGBA-пиксели с учётом вертикального переворота. Различия могут быть в сжатии, разрешении или альфа-канале. Это коллекция для просмотра, а не готовый импорт. Повторы между донорами объединены; все источники перечислены в manifest.json.</p>
<input id="q" placeholder="Поиск: имя, билд, Nanos, Characters, Mobs, Cosmetics"><span id="count"></span><div id="grid">'''+''.join(cards)+'''</div>
<script>const cards=[...document.querySelectorAll('.card')],q=document.querySelector('#q'),count=document.querySelector('#count');function filter(){let n=0;const terms=q.value.toLowerCase().split(/\\s+/).filter(Boolean);for(const card of cards){const show=terms.every(t=>card.dataset.search.includes(t));card.style.display=show?'':'none';n+=show}count.textContent=n+' / '+cards.length}q.addEventListener('input',filter);filter()</script></html>'''
(root/'index.html').write_text(document,encoding='utf-8')
counts=collections.Counter(row['category'] for row in report['textures'])
lines=['# Найденные текстуры фан-клиентов FusionFall','','Откройте `index.html` для поиска и просмотра. PNG открывается кликом по карточке.', '',
       f"Уникальных PNG: {len(report['textures'])}. Категории: {dict(counts)}.", '',
       'Сравнение: текущие PNG из '+report['nativeRoot']+'. Точные RGBA-повторы, включая вертикально перевёрнутые, исключены. Сравнение касается базового изображения, а не материалов, шейдеров, mip-цепочек или готовности к импорту.', '',
       '`Different` — есть кандидат с таким же нормализованным именем, но пиксели отличаются. `NoExactMatch` — точного изображения не найдено; это не утверждение, что самого персонажа нет в игре. Различия могут быть только в сжатии, альфа-канале или разрешении.', '',
       'Повторы между билдами хранятся один раз в папке первого источника. Все места обнаружения, хеши, размеры, Unity-файл и PathID перечислены в manifest.json. Превью находятся в _preview.', '', '## Источники', '']
for src in report['sources']:
    n=sum(any(s['source']==src['source'] for s in row['sources']) for row in report['textures'])
    lines.append(f"- {src['source']}: просмотрено {src['textures']} текстур; {n} уникальных находок участвуют в коллекции; ошибок чтения {src['errors']}.")
lines += ['', 'Ошибки чтения перечислены в manifest.json. Исходные билды и файлы клиента не изменены.']
(root/'README.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
print(dict(counts))
