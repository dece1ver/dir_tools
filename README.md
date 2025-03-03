# DirTools (dirt)

Утилита командной строки для эффективной работы с файлами и директориями.

## Установка

```bash
cargo install dir_tools
# или
cargo install --git https://github.com/dece1ver/dir_tools.git
# или скомпилировать из исходников
git clone https://github.com/dece1ver/dir_tools.git
cd dir_tools
cargo build --release
# или скачать в релизах
```

## Использование

```bash
dirt [КОМАНДА] [ОПЦИИ]
```

## Примеры использования

```bash
# Переименовать все файлы, заменяя "old" на "new"
dirt rename . files --find "old" --replace "new"

# Переместить все файлы из вложенных папок в одну директорию
dirt flatten ./downloads -m -o ./organized

# Добавить имя родительской папки к названию каждого файла
dirt add-parent-dir ./photos -d "_"

# Найти все файлы с текстом "TODO"
dirt find ./project content -p "TODO" -o ./todo-list.txt

# Просмотреть структуру проекта с содержимым
dirt tree . -c -d 3
```

## Подробнее

```bash
dirt -h
```