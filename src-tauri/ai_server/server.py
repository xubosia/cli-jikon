from flask import Flask, request, jsonify
from flask_cors import CORS
from model_loader import ModelManager
import os
import threading
import re

app = Flask(__name__)
CORS(app)
manager = ModelManager()
lock = threading.Lock()

# 模型存放目录（优先使用 Rust 传入的环境变量）
MODELS_DIR = os.environ.get("MODELS_DIR", os.path.join(os.path.dirname(__file__), "models"))


@app.route('/list_models', methods=['GET'])
def list_models():
    models = []
    if os.path.isdir(MODELS_DIR):
        for f in os.listdir(MODELS_DIR):
            if f.endswith('.pth'):
                base = f[:-4]  # 去掉 .pth
                config_path = os.path.join(MODELS_DIR, f"{base}_config.json")
                vocab_path = os.path.join(MODELS_DIR, f"{base}_vocab.json")
                if os.path.exists(config_path) and os.path.exists(vocab_path):
                    models.append(base)
    return jsonify({'models': sorted(models)})


@app.route('/health', methods=['GET'])
def health():
    return jsonify({'status': 'ok', 'model_loaded': manager.model is not None})


@app.route('/predict', methods=['POST'])
def predict():
    data = request.get_json(force=True)
    text = data.get('text', '')
    k = int(data.get('k', 7))
    if not text:
        return jsonify({'candidates': []})
    try:
        with lock:
            candidates = manager.predict(text, k)
        return jsonify({'candidates': candidates})
    except Exception as e:
        return jsonify({'error': str(e)}), 500


@app.route('/set_model', methods=['POST'])
def set_model():
    data = request.get_json(force=True)
    base_name = data.get('model_name', '')
    if not base_name or not re.match(r'^[a-zA-Z0-9_\-]+$', base_name):
        return jsonify({'error': '非法模型名'}), 400

    pth_path = os.path.join(MODELS_DIR, f"{base_name}.pth")
    config_path = os.path.join(MODELS_DIR, f"{base_name}_config.json")
    vocab_path = os.path.join(MODELS_DIR, f"{base_name}_vocab.json")

    if not all(os.path.exists(p) for p in [pth_path, config_path, vocab_path]):
        return jsonify({'error': '模型文件不完整，需要 .pth / _config.json / _vocab.json'}), 404

    try:
        with lock:
            manager.load_model(pth_path, config_path, vocab_path)
        return jsonify({'status': 'ok', 'model': base_name})
    except Exception as e:
        return jsonify({'error': str(e)}), 500


if __name__ == '__main__':
    default_model_name = "haizi"
    pth_path = os.path.join(MODELS_DIR, f"{default_model_name}.pth")
    config_path = os.path.join(MODELS_DIR, f"{default_model_name}_config.json")
    vocab_path = os.path.join(MODELS_DIR, f"{default_model_name}_vocab.json")

    if all(os.path.exists(p) for p in [pth_path, config_path, vocab_path]):
        try:
            manager.load_model(pth_path, config_path, vocab_path)
            print(f"默认模型已加载: {default_model_name}")
        except Exception as e:
            print(f"加载默认模型失败: {e}")
    else:
        print("默认模型文件不完整，请通过 /set_model 加载。")

    app.run(host='127.0.0.1', port=5001, debug=False)