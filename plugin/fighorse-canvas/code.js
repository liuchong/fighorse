figma.showUI(__html__, { width: 420, height: 440 });

const VERSION = "0.0.3";
const nodeByOpId = new Map();
let latestTransactionId = null;

figma.ui.onmessage = async (message) => {
  if (!message || !message.type) return;
  if (message.type === "describe-session") {
    figma.ui.postMessage({ type: "session", session: describeSession() });
    return;
  }
  if (message.type === "plugin-request") {
    const response = await handleRequest(message.command, message.params || {});
    figma.ui.postMessage({
      type: "plugin-response",
      id: message.id,
      result: response.result,
      error: response.error
    });
  }
};

function describeSession() {
  return {
    session_id: `figma-${Date.now()}-${Math.floor(Math.random() * 100000)}`,
    plugin_version: VERSION,
    editor_type: figma.editorType || "figma",
    document_name: figma.root && figma.root.name ? figma.root.name : "Untitled Figma document",
    current_page: figma.currentPage ? figma.currentPage.name : null,
    selection_count: figma.currentPage && figma.currentPage.selection ? figma.currentPage.selection.length : 0,
    capabilities: [
      "inspect",
      "apply",
      "capture",
      "verify",
      "undo",
      "execute_script"
    ]
  };
}

async function handleRequest(command, params) {
  try {
    if (command === "inspect") return ok(inspectCanvas());
    if (command === "apply_plan") return ok(await applyPlan(params));
    if (command === "undo") return ok(await undoTransaction(params.transaction_id));
    if (command === "execute_script") return ok(await executeScript(params));
    return fail("unsupported_operation", `Unsupported command: ${command}`);
  } catch (error) {
    return fail("transport_failed", error && error.message ? error.message : String(error));
  }
}

function ok(result) {
  return { result, error: null };
}

function fail(code, message) {
  return { result: null, error: { code, message } };
}

function currentEditor() {
  return figma.editorType || "figma";
}

function inspectCanvas() {
  const selection = figma.currentPage && figma.currentPage.selection
    ? figma.currentPage.selection.map((node) => summarizeNode(node))
    : [];
  return {
    editor_type: currentEditor(),
    document_name: figma.root.name,
    current_page: figma.currentPage ? summarizeNode(figma.currentPage) : null,
    selection
  };
}

function summarizeNode(node) {
  return {
    id: node.id,
    name: node.name,
    type: node.type,
    x: typeof node.x === "number" ? node.x : null,
    y: typeof node.y === "number" ? node.y : null,
    width: typeof node.width === "number" ? node.width : null,
    height: typeof node.height === "number" ? node.height : null
  };
}

async function applyPlan(plan) {
  const transactionId = plan.transaction_id || `txn-${Date.now()}`;
  const result = {
    transaction_id: transactionId,
    session_id: plan.session_id || "",
    status: "applied",
    operations: [],
    node_ids: []
  };
  try {
    ensureEditor(plan.expected_editor);
    figma.commitUndo();
    for (const operation of plan.operations || []) {
      const opResult = await executeOperation(operation);
      result.operations.push(opResult);
      for (const id of opResult.node_ids || []) {
        result.node_ids.push(id);
      }
    }
    figma.commitUndo();
    latestTransactionId = transactionId;
    return result;
  } catch (error) {
    try {
      figma.triggerUndo();
      result.status = "rolled_back";
    } catch (rollbackError) {
      result.status = "partial";
      result.error = {
        code: "rollback_failed",
        message: rollbackError && rollbackError.message ? rollbackError.message : String(rollbackError)
      };
      return result;
    }
    result.error = {
      code: error.code || "unsupported_operation",
      message: error.message || String(error)
    };
    return result;
  }
}

function ensureEditor(expected) {
  if (expected && expected !== currentEditor()) {
    throw coded("editor_mismatch", `Plan expects ${expected}, current editor is ${currentEditor()}.`);
  }
}

async function executeOperation(operation) {
  const args = operation.args || {};
  const created = [];
  let node = null;
  switch (operation.op) {
    case "create_page":
      node = createPage(args);
      break;
    case "create_frame":
      node = createFrame(args);
      break;
    case "create_section":
      node = createSection(args);
      break;
    case "create_rectangle":
      node = createBasic("createRectangle", args);
      break;
    case "create_ellipse":
      node = createBasic("createEllipse", args);
      break;
    case "create_polygon":
      node = createBasic("createPolygon", args);
      break;
    case "create_line":
      node = createBasic("createLine", args);
      break;
    case "create_text":
      node = await createText(args);
      break;
    case "set_auto_layout":
      node = requireNode(args.node);
      setAutoLayout(node, args);
      break;
    case "create_component":
      node = createBasic("createComponent", args);
      break;
    case "create_instance":
      node = createInstance(args);
      break;
    case "create_variant":
      node = createVariant(args);
      break;
    case "create_variable_collection":
      node = createVariableCollection(args);
      break;
    case "bind_variable":
      node = await bindVariable(args);
      break;
    case "set_style":
      node = setStyle(args);
      break;
    case "create_sticky":
      node = createBasic("createSticky", args);
      break;
    case "create_shape_with_text":
      node = createShapeWithText(args);
      break;
    case "create_connector":
      node = createConnector(args);
      break;
    case "create_table":
      node = createBasic("createTable", args);
      break;
    case "create_code_block":
      node = createBasic("createCodeBlock", args);
      break;
    case "create_slide_row":
      node = createBasic("createSlideRow", args);
      break;
    case "create_slide":
      node = createSlide(args);
      break;
    case "create_shape":
      node = createShape(args);
      break;
    case "create_layout":
      node = createLayout(args);
      break;
    case "create_speaker_notes":
      node = setSpeakerNotes(args);
      break;
    case "set_skip_slide":
      node = setSkipSlide(args);
      break;
    case "reorder_slide":
      node = reorderSlide(args);
      break;
    case "rename_node":
      node = requireNode(args.node);
      node.name = String(args.name || node.name);
      break;
    case "move_node":
      node = requireNode(args.node);
      setGeometry(node, args);
      break;
    case "resize_node":
      node = requireNode(args.node);
      resizeIfPossible(node, args.width, args.height);
      break;
    case "delete_node":
      node = requireNode(args.node);
      node.remove();
      break;
    case "duplicate_node":
      node = requireNode(args.node).clone();
      appendToParent(node, args.parent);
      break;
    case "reparent_node":
      node = requireNode(args.node);
      appendToParent(node, args.parent);
      break;
    case "set_opacity":
      node = requireNode(args.node);
      if ("opacity" in node && typeof args.opacity === "number") node.opacity = args.opacity;
      break;
    case "set_fill":
      node = requireNode(args.node);
      setPaints(node, "fills", args);
      break;
    case "set_stroke":
      node = requireNode(args.node);
      setPaints(node, "strokes", args);
      break;
    case "place_asset":
      node = placeAsset(args);
      break;
    case "verify":
    case "capture":
    case "inspect":
      return {
        op_id: operation.op_id || null,
        op: operation.op,
        status: "applied",
        node_ids: [],
        diagnostic: JSON.stringify(inspectCanvas())
      };
    default:
      throw coded("unsupported_operation", `Unsupported operation: ${operation.op}`);
  }
  if (node) {
    if (node.__fighorse_virtual) {
      if (operation.op_id) nodeByOpId.set(operation.op_id, node.id);
      created.push(node.id);
    } else {
      applyName(node, args);
      setGeometry(node, args);
      if (shouldAppendNode(operation.op)) appendToParent(node, args.parent);
      if (operation.op_id) nodeByOpId.set(operation.op_id, node.id);
      created.push(node.id);
    }
  }
  return {
    op_id: operation.op_id || null,
    op: operation.op,
    status: "applied",
    node_ids: created
  };
}

function shouldAppendNode(op) {
  return op.startsWith("create_") || op === "duplicate_node" || op === "place_asset";
}

function createFrame(args) {
  if (!figma.createFrame) throw coded("unsupported_operation", "createFrame is unavailable.");
  const node = figma.createFrame();
  resizeIfPossible(node, args.width || 100, args.height || 100);
  return node;
}

function createPage(args) {
  if (!figma.createPage) throw coded("unsupported_operation", "createPage is unavailable.");
  const node = figma.createPage();
  if (args.name) node.name = String(args.name);
  if (args.make_current !== false) figma.currentPage = node;
  return node;
}

function createSection(args) {
  if (!figma.createSection) throw coded("unsupported_operation", "createSection is unavailable.");
  const node = figma.createSection();
  resizeIfPossible(node, args.width || 400, args.height || 300);
  return node;
}

function createBasic(method, args) {
  if (!figma[method]) throw coded("unsupported_operation", `${method} is unavailable.`);
  const node = figma[method]();
  resizeIfPossible(node, args.width || 100, args.height || 100);
  return node;
}

function setAutoLayout(node, args) {
  if (!("layoutMode" in node)) {
    throw coded("unsupported_operation", "Auto layout is unavailable for this node.");
  }
  node.layoutMode = args.mode === "vertical" ? "VERTICAL" : "HORIZONTAL";
  if (typeof args.item_spacing === "number") node.itemSpacing = args.item_spacing;
  if (typeof args.padding_left === "number") node.paddingLeft = args.padding_left;
  if (typeof args.padding_right === "number") node.paddingRight = args.padding_right;
  if (typeof args.padding_top === "number") node.paddingTop = args.padding_top;
  if (typeof args.padding_bottom === "number") node.paddingBottom = args.padding_bottom;
}

function createInstance(args) {
  const component = requireNode(args.component || args.node);
  if (typeof component.createInstance !== "function") {
    throw coded("unsupported_operation", "createInstance is unavailable for this component.");
  }
  return component.createInstance();
}

function createVariant(args) {
  const node = createBasic("createComponent", args);
  if (args.properties && "variantProperties" in node) {
    node.variantProperties = args.properties;
  }
  return node;
}

function createVariableCollection(args) {
  const variables = figma.variables;
  if (!variables || typeof variables.createVariableCollection !== "function") {
    throw coded("unsupported_operation", "Variable collection creation is unavailable.");
  }
  const collection = variables.createVariableCollection(String(args.name || "fighorse variables"));
  return {
    id: collection.id,
    name: collection.name,
    type: "VARIABLE_COLLECTION",
    __fighorse_virtual: true
  };
}

async function bindVariable(args) {
  const node = requireNode(args.node);
  const variableId = String(args.variable_id || args.variable || "");
  const field = String(args.field || "fills");
  if (!variableId || typeof node.setBoundVariable !== "function") {
    throw coded("unsupported_operation", "Variable binding is unavailable for this node.");
  }
  let variable = variableId;
  if (figma.variables && typeof figma.variables.getVariableByIdAsync === "function") {
    variable = await figma.variables.getVariableByIdAsync(variableId);
  } else if (figma.variables && typeof figma.variables.getVariableById === "function") {
    variable = figma.variables.getVariableById(variableId);
  }
  if (!variable) throw coded("invalid_plan", `Variable not found: ${variableId}`);
  node.setBoundVariable(field, variable);
  return node;
}

function setStyle(args) {
  const node = requireNode(args.node);
  if (args.fill_style_id && "fillStyleId" in node) node.fillStyleId = String(args.fill_style_id);
  if (args.stroke_style_id && "strokeStyleId" in node) node.strokeStyleId = String(args.stroke_style_id);
  if (args.text_style_id && "textStyleId" in node) node.textStyleId = String(args.text_style_id);
  if (args.effect_style_id && "effectStyleId" in node) node.effectStyleId = String(args.effect_style_id);
  return node;
}

function setPaints(node, field, args) {
  if (!(field in node)) {
    throw coded("unsupported_operation", `${field} is unavailable for this node.`);
  }
  if (Array.isArray(args[field])) {
    node[field] = args[field];
    return;
  }
  if (args.paint) {
    node[field] = [args.paint];
    return;
  }
  if (args.color) {
    const color = args.color;
    node[field] = [{
      type: "SOLID",
      color: {
        r: Number(color.r || 0),
        g: Number(color.g || 0),
        b: Number(color.b || 0)
      },
      opacity: typeof color.a === "number" ? color.a : 1
    }];
    return;
  }
  throw coded("invalid_plan", `${field} requires paint, ${field}, or color.`);
}

async function createText(args) {
  if (!figma.createText) throw coded("unsupported_operation", "createText is unavailable.");
  try {
    await figma.loadFontAsync({ family: args.font_family || "Inter", style: args.font_style || "Regular" });
  } catch (_) {
    await figma.loadFontAsync({ family: "Inter", style: "Regular" }).catch(() => {});
  }
  const node = figma.createText();
  node.characters = String(args.text || "");
  return node;
}

function createShapeWithText(args) {
  const node = createBasic("createShapeWithText", args);
  if ("text" in node) node.text = String(args.text || "");
  if ("characters" in node) node.characters = String(args.text || "");
  return node;
}

function createConnector(args) {
  const node = createBasic("createConnector", args);
  const from = maybeNode(args.from);
  const to = maybeNode(args.to);
  if (from && "connectorStart" in node) node.connectorStart = { endpointNodeId: from.id, magnet: "AUTO" };
  if (to && "connectorEnd" in node) node.connectorEnd = { endpointNodeId: to.id, magnet: "AUTO" };
  return node;
}

function createSlide(args) {
  const node = createBasic("createSlide", args);
  if (args.title && figma.createText) {
    createText({ text: args.title, x: 40, y: 40 }).then((text) => node.appendChild(text));
  }
  return node;
}

function createShape(args) {
  if (figma.createShapeWithText) return createShapeWithText(args);
  return createBasic("createRectangle", args);
}

function createLayout(args) {
  const node = createBasic("createFrame", args);
  if (args.direction) setAutoLayout(node, { ...args, mode: args.direction });
  return node;
}

function setSpeakerNotes(args) {
  const slide = requireNode(args.slide);
  if ("speakerNotes" in slide) {
    slide.speakerNotes = String(args.text || "");
    return slide;
  }
  throw coded("unsupported_operation", "speakerNotes is unavailable for this node.");
}

function setSkipSlide(args) {
  const slide = requireNode(args.slide || args.node);
  if ("skipSlide" in slide) {
    slide.skipSlide = Boolean(args.skip);
    return slide;
  }
  if ("isSkipped" in slide) {
    slide.isSkipped = Boolean(args.skip);
    return slide;
  }
  throw coded("unsupported_operation", "Skip slide is unavailable for this node.");
}

function reorderSlide(args) {
  const slide = requireNode(args.slide || args.node);
  const parent = slide.parent;
  if (!parent || typeof parent.insertChild !== "function") {
    throw coded("unsupported_operation", "Slide reordering is unavailable for this node.");
  }
  parent.insertChild(Number(args.index || 0), slide);
  return slide;
}

function placeAsset(args) {
  if (!args.data_base64 || !args.mime) {
    throw coded("invalid_plan", "place_asset requires embedded data_base64 and mime.");
  }
  const bytes = base64ToBytes(String(args.data_base64));
  if (args.mime === "image/svg+xml") {
    if (!figma.createNodeFromSvg) {
      throw coded("unsupported_operation", "createNodeFromSvg is unavailable.");
    }
    const svgText = new TextDecoder().decode(bytes);
    const node = figma.createNodeFromSvg(svgText);
    resizeIfPossible(node, args.width || node.width, args.height || node.height);
    return node;
  }
  if (!figma.createImage || !figma.createRectangle) {
    throw coded("unsupported_operation", "Bitmap asset placement is unavailable.");
  }
  const image = figma.createImage(bytes);
  const node = figma.createRectangle();
  resizeIfPossible(node, args.width || 100, args.height || 100);
  node.fills = [{ type: "IMAGE", scaleMode: "FILL", imageHash: image.hash }];
  return node;
}

function base64ToBytes(value) {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function executeScript(params) {
  const transactionId = params.transaction_id || `txn-script-${Date.now()}`;
  const script = String(params.script || "");
  const result = {
    transaction_id: transactionId,
    session_id: params.session_id || "",
    status: "applied",
    operations: [],
    node_ids: []
  };
  if (script.length > 65536) {
    result.status = "rejected";
    result.error = {
      code: "output_too_large",
      message: "Script exceeds 64 KiB."
    };
    return result;
  }
  try {
    figma.commitUndo();
    const fn = new Function("figma", "params", `"use strict";\n${script}`);
    const output = await fn(figma, params);
    const text = JSON.stringify(output === undefined ? null : output);
    if (text.length > 20480) {
      throw coded("output_too_large", "Script output exceeds 20 KiB.");
    }
    figma.commitUndo();
    latestTransactionId = transactionId;
    result.data = output === undefined ? null : output;
    return result;
  } catch (error) {
    try {
      figma.triggerUndo();
      result.status = "rolled_back";
    } catch (rollbackError) {
      result.status = "partial";
      result.error = {
        code: "rollback_failed",
        message: rollbackError && rollbackError.message ? rollbackError.message : String(rollbackError)
      };
      return result;
    }
    result.error = {
      code: error.code || "unsupported_operation",
      message: error.message || String(error)
    };
    return result;
  }
}

async function undoTransaction(transactionId) {
  if (!latestTransactionId || latestTransactionId !== transactionId) {
    return {
      transaction_id: transactionId || "",
      session_id: "",
      status: "rejected",
      operations: [],
      node_ids: [],
      error: {
        code: "undo_conflict",
        message: "Only the latest plugin transaction can be undone."
      }
    };
  }
  figma.triggerUndo();
  latestTransactionId = null;
  return {
    transaction_id: transactionId,
    session_id: "",
    status: "rolled_back",
    operations: [],
    node_ids: []
  };
}

function applyName(node, args) {
  if (args.name && "name" in node) node.name = String(args.name);
}

function setGeometry(node, args) {
  if (typeof args.x === "number" && "x" in node) node.x = args.x;
  if (typeof args.y === "number" && "y" in node) node.y = args.y;
}

function resizeIfPossible(node, width, height) {
  if (typeof node.resize === "function" && typeof width === "number" && typeof height === "number") {
    node.resize(width, height);
  }
}

function appendToParent(node, parentRef) {
  if (node.type === "PAGE") return;
  const parent = maybeNode(parentRef);
  if (parent && typeof parent.appendChild === "function" && node.parent !== parent) {
    parent.appendChild(node);
    return;
  }
  if (figma.currentPage && node.parent !== figma.currentPage && typeof figma.currentPage.appendChild === "function") {
    figma.currentPage.appendChild(node);
  }
}

function maybeNode(ref) {
  if (!ref) return null;
  const id = nodeByOpId.get(ref) || ref;
  if (figma.getNodeById) return figma.getNodeById(id);
  return null;
}

function requireNode(ref) {
  const node = maybeNode(ref);
  if (!node) throw coded("invalid_plan", `Node not found: ${ref}`);
  return node;
}

function coded(code, message) {
  const error = new Error(message);
  error.code = code;
  return error;
}
