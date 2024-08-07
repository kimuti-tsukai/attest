# Attest

## Test and Submit your code for AtCoder

## Test
```bash
attest [test | t] <URL>
```

You can omit URL after first test, submit and select lang, and Attest holds examples in `.attest/examples.json`.

If you test with the same code and settings, Attest don't build the program.

You can select example's numbers by following below.
```bash
attest [test | t] <URL> [-n | --num] <Num1> <Num2> ...
```

You make Attest build the program with `-b` or `--build` option.

## Lang Selecting
```bash
attest lang <LANG>
```

Attest enumerates languages by following below.
```bash
attest lang [-l | --list]
```

You can find the language by following below.
```bash
attest lang -s <LANG>
```
You can use partial matching.

You can manage outputs to the certain contest with `[-u | --url] <URL>` option.

## Submit
You can submit your code by following below.
```bash
attest [submit | s] <URL>
```

Please select language before submitting.
Or, you can select language when you submit by following below.
```bash
attest [submit | s] <URL> [-l | --lang] <LANG>
```

## Setting
```bash
attest set <SUBCOMMAND> <VALUE>
```
