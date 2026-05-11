# Example Luma themes

Copy a JSON file into your Luma theme directory and select it by filename stem:

```bash
mkdir -p ~/.config/luma/themes
cp examples/themes/example-dark.json ~/.config/luma/themes/my-dark.json
lumactl theme validate ~/.config/luma/themes/my-dark.json
lumactl config --dark my-dark
```

Custom theme keys are simple filename stems such as `my-dark`. Custom palettes override built-in palettes with the same key.
