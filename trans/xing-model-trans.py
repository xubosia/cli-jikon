import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import Dataset, DataLoader
import jieba
import re
from dataclasses import dataclass
import step0
import time
import json
import os

# ==================== 数据处理 ====================
def take_fraction(lines, fraction=0.1, start_ratio=0.0):
    total = len(lines)# 行数
    start = int(total * start_ratio)# 起始行
    length = int(total * fraction)# 行数
    return lines[start:start+length]
# fraction=0.1：截取的比例，默认值为 0.1（即截取总长度的 10%）。
# start_ratio=0.0：开始截取的位置比例，默认值为 0.0（即从列表的最开头开始）。
# ==================== 配置 ====================
# 原始文本行（全局变量，仅用于训练）
raw_lines = step0.MY_DICT().shi()
raw_lines = take_fraction(raw_lines, fraction=0.3, start_ratio=0.0)  # 只使用前20%的数据进行训练

@dataclass
class Config:
    model_name: str = "shi1"   # 输出文件的基础名
    # 数据
    seq_length: int = 10# 序列长度
    batch_size: int = 1024
    val_split: float = 0.0
    min_freq: int = 8

    # 模型结构
    embed_size: int = 64
    hidden_size: int = 1024
    num_layers: int = 1

    # 关闭所有正则化
    embed_dropout: float = 0.0
    gru_dropout: float = 0.0
    out_dropout: float = 0.0
    weight_decay: float = 0.0

    # 优化器
    lr: float = 1e-3
    epochs: int = 1000

    # 预测
    topk: int = 7
    temperature: float = 1.0

    # 设备
    device: torch.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")




def build_vocab_and_data(cfg):
    raw_text = " ".join(raw_lines)
    words = list(jieba.cut(raw_text))

    word_counts = {}
    for w in words:
        word_counts[w] = word_counts.get(w, 0) + 1

    vocab = {'<PAD>': 0, '<UNK>': 1}
    for w in words:
        if w not in vocab and word_counts[w] >= cfg.min_freq:
            vocab[w] = len(vocab)

    idx_to_word = {i: w for w, i in vocab.items()}
    indices = [vocab.get(w, vocab['<UNK>']) for w in words]
    return indices, vocab, idx_to_word


class TextDataset(Dataset):
    def __init__(self, data, seq_length):
        self.data = torch.tensor(data, dtype=torch.long)
        self.seq_length = seq_length

    def __len__(self):
        return len(self.data) - self.seq_length

    def __getitem__(self, idx):
        return self.data[idx:idx+self.seq_length], self.data[idx+self.seq_length]


def prepare_dataloaders(indices, cfg):
    n_val = int(len(indices) * cfg.val_split)
    train_data = indices[:-n_val] if n_val > 0 else indices
    train_ds = TextDataset(train_data, cfg.seq_length)
    train_loader = DataLoader(train_ds, batch_size=cfg.batch_size,
                              shuffle=True, drop_last=True, num_workers=0)
    return train_loader


# ==================== 模型：GRU ====================
class RNNGru(nn.Module):
    def __init__(self, vocab_size, cfg):
        super().__init__()
        self.embed = nn.Embedding(vocab_size, cfg.embed_size)
        self.embed_drop = nn.Dropout(cfg.embed_dropout)
        self.gru = nn.GRU(cfg.embed_size, cfg.hidden_size, cfg.num_layers,
                          batch_first=True, dropout=cfg.gru_dropout)
        self.out_drop = nn.Dropout(cfg.out_dropout)
        self.fc = nn.Linear(cfg.hidden_size, vocab_size)
        self.num_layers = cfg.num_layers
        self.hidden_size = cfg.hidden_size
        self.device = cfg.device

    def forward(self, x, hidden):
        x = self.embed(x)
        x = self.embed_drop(x)
        out, hidden = self.gru(x, hidden)
        out = self.out_drop(out[:, -1, :])
        return self.fc(out), hidden

    def init_hidden(self, batch_size):
        return torch.zeros(self.num_layers, batch_size, self.hidden_size, device=self.device)


# ==================== 训练器 ====================
class Trainer:
    def __init__(self, model, train_loader, cfg, vocab):
        self.model = model.to(cfg.device)
        self.train_loader = train_loader
        self.cfg = cfg
        self.vocab = vocab              # 词表字典（str->int）
        self.criterion = nn.CrossEntropyLoss()
        self.optimizer = optim.Adam(model.parameters(), lr=cfg.lr)
        self.scaler = torch.amp.GradScaler(cfg.device.type) if cfg.device.type == 'cuda' else None

    def run_epoch(self):
        self.model.train()
        total_loss, n_batches = 0.0, len(self.train_loader)
        for x, y in self.train_loader:
            x, y = x.to(self.cfg.device), y.to(self.cfg.device)
            hidden = self.model.init_hidden(x.size(0))

            if self.scaler is not None:
                with torch.amp.autocast(self.cfg.device.type):
                    out, _ = self.model(x, hidden)
                    loss = self.criterion(out, y)
            else:
                out, _ = self.model(x, hidden)
                loss = self.criterion(out, y)

            self.optimizer.zero_grad()
            if self.scaler is not None:
                self.scaler.scale(loss).backward()
                self.scaler.step(self.optimizer)
                self.scaler.update()
            else:
                loss.backward()
                self.optimizer.step()

            total_loss += loss.item()
        return total_loss / n_batches

    def save_model_files(self):
        """保存 .pth, _config.json, _vocab.json 三个文件"""
        base = self.cfg.model_name

        # 1. 模型权重
        weight_path = f"{base}.pth"
        torch.save(self.model.state_dict(), weight_path)
        print(f"模型权重已保存: {weight_path}")

        # 2. 模型结构配置
        config = {
            "vocab_size": len(self.vocab),
            "embed_size": self.cfg.embed_size,
            "hidden_size": self.cfg.hidden_size,
            "num_layers": self.cfg.num_layers,
            "seq_length": self.cfg.seq_length,
            "min_freq": self.cfg.min_freq,
        }
        config_path = f"{base}_config.json"
        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, ensure_ascii=False, indent=2)
        print(f"模型配置已保存: {config_path}")

        # 3. 词表映射（直接保存字典，不再依赖原始语料）
        vocab_json_path = f"{base}_vocab.json"
        with open(vocab_json_path, "w", encoding="utf-8") as f:
            json.dump(self.vocab, f, ensure_ascii=False)
        print(f"词表映射已保存: {vocab_json_path}")

    def fit(self):
        try:
            for epoch in range(self.cfg.epochs):
                epoch_start = time.time()
                train_loss = self.run_epoch()
                epoch_time = time.time() - epoch_start
                print(f"Epoch {epoch+1:4d}/{self.cfg.epochs} | train loss: {train_loss:.6f} | 耗时: {epoch_time:.2f}s")
            print("训练正常结束，正在保存模型文件...")
            self.save_model_files()
        except KeyboardInterrupt:
            print("\n训练被手动中断，正在保存当前模型...")
            self.save_model_files()


# ==================== 预测 ====================
def get_candidates(model, vocab, idx_to_word, text, cfg):
    model.eval()
    with torch.no_grad():
        tokens = list(jieba.cut(text))
        indices = [vocab.get(w, vocab['<UNK>']) for w in tokens]
        if len(indices) < cfg.seq_length:
            indices = [0] * (cfg.seq_length - len(indices)) + indices
        else:
            indices = indices[-cfg.seq_length:]
        inp = torch.tensor([indices], dtype=torch.long, device=cfg.device)
        hidden = model.init_hidden(1)
        out, _ = model(inp, hidden)
        logits = out / cfg.temperature
        probs = torch.softmax(logits, dim=-1)
        probs, top_idx = torch.topk(probs, cfg.topk)
        return [(idx_to_word[top_idx[0][i].item()], probs[0][i].item()) for i in range(cfg.topk)]


# ==================== 主程序 ====================
if __name__ == "__main__":
    cfg = Config()
    print(f"设备: {cfg.device}")
    print(f"输出文件基础名: {cfg.model_name}")

    # 数据准备
    indices, vocab, idx_to_word = build_vocab_and_data(cfg)
    print(f"数据集大小: {len(indices)}")
    print(f"词表大小: {len(vocab)}")
    train_loader = prepare_dataloaders(indices, cfg)

    model = RNNGru(len(vocab), cfg)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"\n模型总参数量: {total_params:,}")
    print("各层参数量：")
    for name, module in model.named_modules():
        module_params = list(module.parameters(recurse=False))
        if len(module_params) == 0:
            continue
        num = sum(p.numel() for p in module_params)
        print(f"  {name:15s} : {num:>10,}")

    trainer = Trainer(model, train_loader, cfg, vocab)
    trainer.fit()

    # 交互预测
    print("\n" + "="*40)
    print("输入法测试 (输入 exit 退出)")
    current = ""
    while True:
        print(f"\n输入框: {current}")
        cands = get_candidates(model, vocab, idx_to_word, current, cfg)
        print("候选: " + " ".join([f"{i+1}.{w}" for i, (w, _) in enumerate(cands)]))
        choice = input("选择: ")
        if choice == 'exit':
            break
        if choice.isdigit() and 1 <= int(choice) <= cfg.topk:
            current += cands[int(choice)-1][0]
        else:
            current += choice