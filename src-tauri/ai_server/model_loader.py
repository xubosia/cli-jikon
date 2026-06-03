import torch
import torch.nn as nn
import jieba
import re
import os

# ================== 模型定义（与训练时完全一致） ==================
class RNNGru(nn.Module):
    def __init__(self, vocab_size, embed_size, hidden_size, num_layers=1):
        super().__init__()
        self.embed = nn.Embedding(vocab_size, embed_size)
        self.gru = nn.GRU(embed_size, hidden_size, num_layers,
                          batch_first=True, dropout=0.0)
        self.fc = nn.Linear(hidden_size, vocab_size)
        self.num_layers = num_layers
        self.hidden_size = hidden_size

    def forward(self, x, hidden):
        x = self.embed(x)
        out, hidden = self.gru(x, hidden)
        out = self.fc(out[:, -1, :])
        return out, hidden

    def init_hidden(self, batch_size, device):
        return torch.zeros(self.num_layers, batch_size, self.hidden_size, device=device)



# ================== 全局模型管理 ==================
class ModelManager:
    def __init__(self):
        self.model = None
        self.vocab = None
        self.idx_to_word = None
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.cfg = {
            'embed_size': 32,
            'hidden_size': 1024,
            'num_layers': 1,
            'seq_length': 20
        }

    

    def load_model(self, pth_path, config_path, vocab_json_path):
        """加载模型权重、配置、词表（_vocab.json 格式）"""
        import json

        # 1. 读配置
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
        self.cfg.update({
            'embed_size': config['embed_size'],
            'hidden_size': config['hidden_size'],
            'num_layers': config.get('num_layers', 1),
            'seq_length': config['seq_length'],          # 别忘了 seq_length
        })

        # 2. 读词表字典
        with open(vocab_json_path, 'r', encoding='utf-8') as f:
            vocab = json.load(f)
        # vocab 是 {词: 索引}，JSON 里 key 是字符串，value 是数字
        vocab = {w: int(i) for w, i in vocab.items()}  # 确保索引为 int
        idx_to_word = {int(i): w for w, i in vocab.items()}
        vocab_size = len(vocab)

        # 3. 创建模型
        model = RNNGru(
            vocab_size,
            config['embed_size'],
            config['hidden_size'],
            config.get('num_layers', 1)
        ).to(self.device)

        # 4. 加载权重
        state_dict = torch.load(pth_path, map_location=self.device, weights_only=True)
        model.load_state_dict(state_dict)
        model.eval()

        self.model = model
        self.vocab = vocab
        self.idx_to_word = idx_to_word
        print(f"模型已加载: {pth_path}, 词表大小: {vocab_size}")
    def predict(self, text, k=7, temperature=1.0):
        if self.model is None:
            raise RuntimeError("模型未加载")
        tokens = list(jieba.cut(text))
        indices = [self.vocab.get(w, self.vocab['<UNK>']) for w in tokens]
        if len(indices) < self.cfg['seq_length']:
            indices = [0] * (self.cfg['seq_length'] - len(indices)) + indices
        else:
            indices = indices[-self.cfg['seq_length']:]

        inp = torch.tensor([indices], dtype=torch.long, device=self.device)
        hidden = self.model.init_hidden(1, self.device)
        with torch.no_grad():
            out, _ = self.model(inp, hidden)
            logits = out / temperature
            probs = torch.softmax(logits, dim=-1)
            top_probs, top_idx = torch.topk(probs, k)

        result = []
        for i in range(k):
            word = self.idx_to_word[top_idx[0][i].item()]
            prob = top_probs[0][i].item()
            result.append({'word': word, 'prob': prob})
        return result