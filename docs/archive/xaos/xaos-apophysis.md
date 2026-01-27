# XAOS Implementation in Apophysis 7X

This document provides a comprehensive technical reference for how XAOS (weighted transform transitions) is implemented in Apophysis 7X.

## Overview

XAOS allows asymmetric weighting of transitions between transforms in the chaos game iteration. Instead of selecting the next transform based solely on its density weight, XAOS adds per-source-transform modifiers that control how likely each destination transform is to be selected.

**Key Concept:** When iterating from transform A, the probability of jumping to transform B is proportional to `density[B] * xaos[A→B]`.

---

## 1. Data Structures

### Primary Storage: `modWeights` Array

**Location:** [XForm.pas:96](src/Flame/XForm.pas#L96)

```pascal
modWeights: array [0..NXFORMS] of double;
```

| Property | Value |
|----------|-------|
| Type | Array of doubles |
| Size | NXFORMS + 1 elements (101 or 501 depending on build) |
| Default | 1.0 for all indices |
| Range | 0.0 to any positive value (no upper limit) |

**Initialization:** [XForm.pas:292-293](src/Flame/XForm.pas#L292-L293)

```pascal
for i := 0 to NXFORMS do
  modWeights[i] := 1;
```

### Conceptual Matrix

XAOS forms an N×N matrix where:
- Rows = source transforms (where we're coming FROM)
- Columns = destination transforms (where we're going TO)
- Cell value = weight modifier for that transition

```
         To Transform
         0    1    2    3
From  0 [1.0, 0.5, 1.0, 0.0]   ← xform[0].modWeights[]
      1 [0.0, 1.0, 2.0, 1.0]   ← xform[1].modWeights[]
      2 [1.0, 1.0, 1.0, 1.0]   ← xform[2].modWeights[]
      3 [0.5, 0.5, 0.5, 1.0]   ← xform[3].modWeights[]
```

---

## 2. Chaos Game Iteration (Rendering)

### PropTable Construction

The renderer builds a probability lookup table for O(1) weighted random selection.

**Location:** [ControlPoint.pas:434-457](src/Flame/ControlPoint.pas#L434-L457)

```pascal
procedure TControlPoint.prepare_xform_weights;
var
  k, i, j: integer;
  totValue: double;
  tp: array of double;
begin
  SetLength(tp, n);

  // For each source transform (k)
  for k := 0 to n - 1 do begin
    totValue := 0;

    // Calculate combined weight for each destination transform (i)
    for i := 0 to n - 1 do begin
      tp[i] := xform[i].density * xform[k].modWeights[i];  // KEY FORMULA
      totValue := totValue + tp[i];
    end;

    // Fill PropTable with 1024 slots proportionally
    j := 0;
    for i := 0 to PROP_TABLE_SIZE - 1 do begin
      // Allocate slots proportional to tp[j] / totValue
      xform[k].PropTable[i] := xform[j];
    end;
  end;
end;
```

**Key Formula:** `probability[k→i] = xform[i].density × xform[k].modWeights[i]`

### Runtime Selection

**Location:** [RenderingImplementation.pas:306](src/Rendering/RenderingImplementation.pas#L306)

```pascal
xf := xf.PropTable[Random(PROP_TABLE_SIZE)];
```

The iteration simply picks a random slot (0-1023) from the current transform's PropTable, giving O(1) weighted random selection.

### Constants

| Constant | Value | Location |
|----------|-------|----------|
| PROP_TABLE_SIZE | 1024 | [ControlPoint.pas:28](src/Flame/ControlPoint.pas#L28) |
| FUSE | 15 | Initial iterations discarded |
| SUB_BATCH_SIZE | 10000 | Iterations per batch |

---

## 3. User Interface

### Chaos Grid Editor

The XAOS editor is integrated into the Transform Editor form.

**Location:** [Editor.pas](src/Forms/Editor.pas)

**Control:** `vleChaos` (TValueListEditor component)

### Display Modes

The grid supports two viewing perspectives:

1. **"View To" Mode** ([Editor.pas:991-997](src/Forms/Editor.pas#L991-L997))
   - Shows weights FROM selected transform TO all others
   - Displays `xform[SelectedTriangle].modWeights[i]`

2. **"View From" Mode** ([Editor.pas:998-1004](src/Forms/Editor.pas#L998-L1004))
   - Shows weights FROM all transforms TO selected transform
   - Displays `xform[i].modWeights[SelectedTriangle]`

### Grid Population

**ShowSelectedInfo()** - [Editor.pas:990-1014](src/Forms/Editor.pas#L990-L1014)

```pascal
if mnuChaosViewTo.Checked then
  // View as "to" values - row of matrix
  for i := 1 to Transforms do
    strval := Format('%.6g', [modWeights[i - 1]])
else
  // View as "from" values - column of matrix
  for i := 1 to Transforms do
    strval := Format('%.6g', [cp.xform[i - 1].modWeights[SelectedTriangle]]);
```

### Value Editing

**Cell Selection Handler:** [Editor.pas:5485-5516](src/Forms/Editor.pas#L5485-L5516)

```pascal
procedure TEditForm.vleChaosSelectCell(...);
begin
  if mnuChaosViewTo.Checked then
    OldVal := cp.xform[SelectedTriangle].modWeights[i]
  else
    OldVal := cp.xform[i].modWeights[SelectedTriangle];

  NewVal := StrToFloat(vleChaos.Cells[1, i+1]);

  // Apply change with undo support
  cp.xform[SelectedTriangle].modWeights[i] := NewVal;
end;
```

### Mouse Drag Interaction

**Drag Start:** [Editor.pas:3748-3752](src/Forms/Editor.pas#L3748-L3752)

```pascal
if mnuChaosViewTo.Checked then
  pDragValue := @cp.xform[SelectedTriangle].modWeights[varDragIndex]
else
  pDragValue := @cp.xform[varDragIndex].modWeights[SelectedTriangle];
```

**Drag Update with Sensitivity Modifiers:** [Editor.pas:3775-3792](src/Forms/Editor.pas#L3775-L3792)

| Modifier | Sensitivity Multiplier |
|----------|----------------------|
| Alt | 100,000× |
| Ctrl | 10,000× |
| Shift | 100× |
| None | 1,000× |

**Double-Click Toggle:** [Editor.pas:3870-3887](src/Forms/Editor.pas#L3870-L3887)

```pascal
v := ifthen(v = 1, 0, 1);  // Toggle between 0 and 1
```

### Menu Commands

| Command | Location | Action |
|---------|----------|--------|
| Clear All Chaos | [Editor.pas:5604-5632](src/Forms/Editor.pas#L5604-L5632) | Set all weights to 0 |
| Set All Chaos | [Editor.pas:5634-5662](src/Forms/Editor.pas#L5634-L5662) | Set all weights to 1.0 |
| View To / View From | [Editor.pas:5567-5587](src/Forms/Editor.pas#L5567-L5587) | Toggle view mode |

---

## 4. File I/O

### XML Export

**Location:** [XForm.pas:1419-1430](src/Flame/XForm.pas#L1419-L1430)

```pascal
function TXForm.ToXMLString: string;
var
  numChaos, i: integer;
begin
  // Find last non-default value for optimization
  numChaos := -1;
  for i := NXFORMS-1 downto 0 do
    if modWeights[i] <> 1 then begin
      numChaos := i;
      break;
    end;

  // Only write if there are non-default values
  if numChaos >= 0 then begin
    Result := Result + 'chaos="';
    for i := 0 to numChaos do
      Result := Result + Format('%g ', [modWeights[i]]);
    Result := Result + '" ';
  end;
end;
```

**XML Format:**
```xml
<xform weight="0.5" color="0" ... chaos="1 0 0.5 1 " />
```

**Optimization:** Only writes values up to the last non-1.0 weight to minimize file size.

### XML Import

**Primary Parser:** [ParameterIO.pas:400-411](src/IO/ParameterIO.pas#L400-L411)

```pascal
if (attrib_name = 'chaos') and (not isFinalXform) then begin
  token_part := GetStringPart(String(attrib_match), re_attrib, 2, '');
  if token_part <> '' then begin
    t := TStringList.Create;
    GetTokens(token_part, t);
    for i := 0 to t.Count-1 do
      xf.modWeights[i] := Abs(StrToFloat(t[i]));  // Note: Abs() ensures positive
    t.Destroy;
  end;
end;
```

**Alternative Parser:** [Main.pas:5393-5399](src/Forms/Main.pas#L5393-L5399)

```pascal
v := Attributes.Value('chaos');
if v <> '' then begin
  GetTokens(String(v), tokens);
  for i := 0 to Tokens.Count-1 do
    modWeights[i] := Abs(StrToFloat(Tokens[i]));
end;
```

**Important:** The `Abs()` call ensures all loaded values are non-negative.

### String List Format (Legacy/Binary)

**Location:** [ControlPoint.pas:1944-1948](src/Flame/ControlPoint.pas#L1944-L1948)

```pascal
s := 'chaos';
for j := 0 to NumXForms+1 do
  s := s + format(' %g', [modWeights[j]]);
sl.Add(s);
```

Format: `chaos 1 0.5 0 1 ...`

---

## 5. Transform Operations

### Transform Deletion

When a transform is deleted, XAOS weights must shift to maintain consistency.

**Location:** [Editor.pas:1213-1215](src/Forms/Editor.pas#L1213-L1215)

```pascal
// For each transform, shift its modWeights array
for j := t to Transforms-1 do
  modWeights[j] := modWeights[j+1];
modWeights[Transforms-1] := 1;  // Reset last to default
```

This effectively removes a column from the conceptual matrix and shifts remaining columns left.

### Transform Duplication

When a transform is duplicated, both the row and column must be copied.

**Location:** [Editor.pas:2837-2838](src/Forms/Editor.pas#L2837-L2838)

```pascal
// Copy column: how other transforms jump TO the new one
cp.xform[i].modWeights[Transforms] := cp.xform[i].modWeights[SelectedTriangle];

// Copy row: how the new transform jumps TO others
cp.xform[Transforms].modWeights[Transforms] :=
  cp.xform[SelectedTriangle].modWeights[SelectedTriangle];
```

### Transform Assignment

**Location:** [XForm.pas:1383-1384](src/Flame/XForm.pas#L1383-L1384)

When copying one xform to another, the entire modWeights array is copied:

```pascal
for i := 0 to NXFORMS do
  modWeights[i] := xf.modWeights[i];
```

---

## 6. Scripting API

XAOS values can be accessed and modified via the scripting system.

### Getter

**Location:** [ScriptForm.pas:2914-2918](src/Forms/ScriptForm.pas#L2914-L2918)

```pascal
procedure TScriptEditor.GetTransformChaosProc(AMachine: TatVirtualMachine);
begin
  with AMachine do
    ReturnOutPutArg(cp.xform[ActiveTransform].modWeights[Integer(GetArrayIndex(0))]);
end;
```

**Script Usage:** `value := Transform.Chaos[targetIndex]`

### Setter

**Location:** [ScriptForm.pas:2920-2932](src/Forms/ScriptForm.pas#L2920-L2932)

```pascal
procedure TScriptEditor.SetTransformChaosProc(AMachine: TatVirtualMachine);
var
  v: double;
  i: integer;
begin
  with AMachine do begin
    v := GetInputArgAsFloat(0);
    i := GetArrayIndex(0);
    if (i >= 0) and (i < NumTransforms) then
      cp.xform[ActiveTransform].modWeights[i] := v;
  end;
end;
```

**Script Usage:** `Transform.Chaos[targetIndex] := 0.5`

### Script Transform Deletion

**Location:** [ScriptForm.pas:2050-2055](src/Forms/ScriptForm.pas#L2050-L2055)

```pascal
for i := 0 to NumTransforms-1 do
  with scriptEditor.cp.xform[i] do begin
    for j := ActiveTransform to NumTransforms-1 do
      modWeights[j] := modWeights[j+1];
    modWeights[NumTransforms-1] := 1;
  end;
```

---

## 7. Key Technical Notes

### No Explicit Normalization

XAOS weights are stored as absolute values and are NOT normalized before storage. Normalization happens implicitly during PropTable construction when the total weight is calculated and slots are allocated proportionally.

### Asymmetric Nature

XAOS is inherently asymmetric: the weight from A→B can differ from B→A. This allows for directional flow control in the chaos game.

### Final Transform Exclusion

The condition `(not isFinalXform)` in the parser ([ParameterIO.pas:400](src/IO/ParameterIO.pas#L400)) prevents XAOS from being applied to the final transform, which is rendered differently.

### Default Behavior

When all weights are 1.0 (default), the transition probability depends only on transform density, giving the classic chaos game behavior.

### Value Semantics

| Weight | Effect |
|--------|--------|
| 0.0 | Never transition to this target |
| 1.0 | Default (no modification) |
| > 1.0 | Increased probability to this target |
| < 1.0 | Decreased probability to this target |

---

## 8. Code Location Reference

| Component | File | Lines |
|-----------|------|-------|
| Data structure | [XForm.pas](src/Flame/XForm.pas) | 96 |
| Initialization | [XForm.pas](src/Flame/XForm.pas) | 292-293 |
| PropTable build | [ControlPoint.pas](src/Flame/ControlPoint.pas) | 434-457 |
| PropTable use | [RenderingImplementation.pas](src/Rendering/RenderingImplementation.pas) | 306 |
| XML export | [XForm.pas](src/Flame/XForm.pas) | 1419-1430 |
| XML import | [ParameterIO.pas](src/IO/ParameterIO.pas) | 400-411 |
| Grid display | [Editor.pas](src/Forms/Editor.pas) | 990-1014 |
| Value editing | [Editor.pas](src/Forms/Editor.pas) | 5485-5516 |
| Drag interaction | [Editor.pas](src/Forms/Editor.pas) | 3748-3792 |
| Clear all | [Editor.pas](src/Forms/Editor.pas) | 5604-5632 |
| Set all | [Editor.pas](src/Forms/Editor.pas) | 5634-5662 |
| Delete transform | [Editor.pas](src/Forms/Editor.pas) | 1213-1215 |
| Copy transform | [Editor.pas](src/Forms/Editor.pas) | 2837-2838 |
| Script getter | [ScriptForm.pas](src/Forms/ScriptForm.pas) | 2914-2918 |
| Script setter | [ScriptForm.pas](src/Forms/ScriptForm.pas) | 2920-2932 |
