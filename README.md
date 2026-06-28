# yp-image-processor

Крейт `image_processor` обеспечивает обработку изображений с использованием плагинов.
Плагины представлят собой динамические библиотеки, которые по-умолчанию собираются вместе с основным крейтом в `./target/debug/`

Реализованы плагины:
- `blur`: размытие
- `mirror`: отражение

Параметры плагинов передаются в виде пути до файла с параметрами в формате JSON, лишние параметры в файле игнорируются.

Пример файла параметров для `blur` со всеми доступными параметрами:
```txt
{"radius": 10, "iterations": 4}
```

Пример файла параметров для `mirror` со всеми доступными параметрами:
```txt
{"horizontal": true, "vertical": true}
```

Примеры команд запуска:

`cargo run --bin image_processor -- --input ./test.png --output ./result.png --params ./params.txt --plugin mirror`

`cargo run --bin image_processor -- --input ./test.png --output ./result.png --params ./params.txt --plugin blur`