# Alas
Alas is a tool that helps you synchronize your LaTeX notes with [Anki](https://apps.ankiweb.net/).

![Alas Demo](demo.gif)

## Requirements
Alas requires [`latex`](https://www.latex-project.org/get/) and [`dvisvgm`](https://dvisvgm.de/) to be installed and accessible in your system's PATH.

## Usage
### 1. Initialize the project directory
To set up Alas in your LaTeX project directory, run the following command:
```
alas init -p <ANKI PROFILE> -d <DECK NAME>
```

### 2. Synchronize changes
After initialization, synchronize any changes between your LaTeX notes and Anki by running:
```
alas sync
```

### Additional Commands and Options
```
Initialize alas for the current directory
Usage: alas init [OPTIONS] --profile <PROFILE>
Options:
  -p, --profile <PROFILE>        Specify the Anki profile for synchronization
  -d, --deck <DECK>              Specify the name of the Anki deck
  -i, --identifier <IDENTIFIER>  Specify the technical name for Anki objects
  -f, --files                    Add template .tex files
  -h, --help                     Print help

Sync all your LaTeX notes with Anki
Usage: alas sync [OPTIONS]
Options:
  -b, --batch-size <BATCH_SIZE>  Specify the batch size [default: 9]
  -h, --help                     Print help
```

## Project structure
Alas assumes your project directory follows a specific structure:
```
project/
├── <your .tex files>
├── preamble.tex
└── preamble_course.tex
```
where `preamble.tex` and `preamble_course.tex` contains latex code inserted before rendering your latex notes. *Tipp: Use the flag `-f` flag when initializing the project to automatically create these files.*

## LaTeX Note Structure
Alas looks for notes in the following format across all `.tex` files:
```latex
\begin{note}
    \begin{field}
        % Front of the flashcard
    \end{field}%
    \begin{field}
        % Back of the flashcard
    \end{field}
\end{note}
```