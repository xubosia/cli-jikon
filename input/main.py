import tkinter as tk
import requests
import math
import re

# 模型 API 地址
API_URL = "http://127.0.0.1:8080/v1/chat/completions"

def is_chinese_token(tok):
    return bool(re.search(r'[\u4e00-\u9fff\u3000-\u303f\uff00-\uffef]', tok))

def get_candidates(text):
    if not text.strip():
        return []
    try:
        r = requests.post(API_URL, json={
            "messages": [
                {"role": "system", "content": "你是一个中文输入法助手，只输出下一个可能的中文词语或汉字，不要其他语言。"},
                {"role": "user", "content": text}
            ],
            "max_tokens": 1,
            "temperature": 0.6,
            "top_p": 0.9,
            "logprobs": True,
            "top_logprobs": 6
        })
        data = r.json()
        top = data['choices'][0]['logprobs']['top_logprobs'][0]
        items = []
        for tok, logp in top.items():
            word = tok.strip()
            if is_chinese_token(word):
                items.append((word, math.exp(logp)))
        items.sort(key=lambda x: x[1], reverse=True)
        return items[:6]
    except:
        return []

class App:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("输入法调试 - 无冲突")
        self.root.geometry("600x200")
        
        # 输入框
        self.entry = tk.Entry(self.root, font=("Microsoft YaHei", 14))
        self.entry.pack(pady=10, padx=20, fill='x')
        self.entry.bind("<KeyRelease>", self.on_key_release)
        self.entry.bind("<Return>", self.on_enter)
        self.entry.bind("<BackSpace>", self.on_key_release)
        self.entry.focus_set()
        
        # 候选栏
        self.candidate_frame = tk.Frame(self.root)
        self.candidate_frame.pack(pady=5)
        
        self.candidate_labels = []
        for i in range(6):
            lbl = tk.Label(self.candidate_frame, text="", font=("Microsoft YaHei", 12),
                           bg="#f0f0f0", padx=6, pady=2, borderwidth=1, relief="solid")
            lbl.grid(row=0, column=i, padx=2)
            lbl.bind("<Button-1>", lambda e, idx=i: self.select_candidate(idx))
            self.candidate_labels.append(lbl)
        
        # 数字键绑定（1-6）
        for i in range(1, 7):
            self.root.bind(str(i), lambda e, idx=i-1: self.select_candidate(idx))
        
        self.after_id = None
        self.update_candidates()
    
    def on_key_release(self, event=None):
        # 防抖 200ms
        if self.after_id:
            self.root.after_cancel(self.after_id)
        self.after_id = self.root.after(200, self.update_candidates)
    
    def update_candidates(self):
        text = self.entry.get()
        candidates = get_candidates(text)
        for i, lbl in enumerate(self.candidate_labels):
            if i < len(candidates):
                word, prob = candidates[i]
                lbl.config(text=f"{i+1}.{word} ({prob:.2f})")
            else:
                lbl.config(text="")
    
    def select_candidate(self, idx):
        candidates = get_candidates(self.entry.get())
        if idx < len(candidates):
            word = candidates[idx][0]
            self.entry.insert(tk.END, word)
            self.update_candidates()
    
    def on_enter(self, event):
        text = self.entry.get()
        print(f">>> 发送: {text}")
        self.entry.delete(0, tk.END)
        self.update_candidates()
    
    def run(self):
        self.root.mainloop()

if __name__ == "__main__":
    App().run()