class MY_DICT:
    def __init__(self):
        pass


    def haizi(self):
        path = r"C:\Users\Administrator\Desktop\个人资料库\海子诗全集海子zlibrarysk1libskzlibsk.txt"

        with open(path, "r", encoding='utf-8') as f:

            lines = [line.replace('\u3000', '').strip() for line in f if line.replace('\u3000', '').strip()]

        return lines
    def shi(self):
        path = r"D:\change-world\programme\真-智能输入法\poems_for_training.txt"

        with open(path, "r", encoding='utf-8') as f:

            lines = [line.replace('\u3000', '').strip() for line in f if line.replace('\u3000', '').strip()]

        return lines
if __name__ == '__main__':

    shuju=MY_DICT().haizi()
    open("haizi.txt", "w", encoding='utf-8').write("\n".join(shuju))

