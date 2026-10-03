"""Lossless Rust item scanner for generated protocol modules.
Does not type-check Rust. Recognises strings/comments and balanced delimiters.
"""
from dataclasses import dataclass
import re

@dataclass(frozen=True)
class Token:
    text: str
    start: int
    end: int

TOKEN_RE = re.compile(r'''(?P<space>\s+)|(?P<line>//[^\n]*)|(?P<block>/\*)|(?P<raw>(?:br|cr|r)(\#*)")|(?P<string>(?:b|c)?"(?:[^"\\]|\\[\s\S])*")|(?P<char>b?'(?:[^'\\\n]|\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|[^\n]))')|(?P<ident>(?:r\#)?[^\W\d]\w*)|(?P<number>[0-9][0-9a-zA-Z_]*(?:\.[0-9][0-9a-zA-Z_]*)?)|(?P<punct>::|->|=>|\.\.=|\.\.|&&|\|\||<<|>>|<=|>=|==|!=|[^\s])''', re.UNICODE)

def tokens(s):
    out=[];pos=0
    while pos<len(s):
        m=TOKEN_RE.match(s,pos)
        if not m:raise ValueError(('unlexed',pos,s[pos:pos+40]))
        kind=m.lastgroup; end=m.end()
        if kind=='block':
            depth=1
            while depth:
                next_open=s.find('/*',end);next_close=s.find('*/',end)
                if next_close<0:raise ValueError(('unterminated comment',pos))
                if 0<=next_open<next_close:depth+=1;end=next_open+2
                else:depth-=1;end=next_close+2
        elif kind=='raw':
            # Last named group is raw even though the hash capture is unnamed.
            hashes=m.group('raw').split('r',1)[1][:-1]
            close='"'+hashes;found=s.find(close,end)
            if found<0:raise ValueError(('unterminated raw string',pos))
            end=found+len(close)
        if kind not in ('space','line','block'):
            out.append(Token(s[pos:end],pos,end))
        pos=end
    return out

def brackets(ts):
    pairs={};stack=[]
    for i,t in enumerate(ts):
        if t.text in ('{','[','('):stack.append(i)
        elif t.text in ('}',']',')'):
            if not stack:raise ValueError(('unmatched closer',t))
            j=stack.pop()
            if {'{':'}','[':']','(':')'}[ts[j].text]!=t.text:raise ValueError(('bracket mismatch',ts[j],t))
            pairs[j]=i;pairs[i]=j
    if stack:raise ValueError(('unclosed delimiter',ts[stack[-1]]))
    return pairs

@dataclass
class Item:
    start: int
    code_start: int
    end: int
    kind: str
    name: str
    sig_start: int
    vis_start: int
    vis_end: int
    visibility: str
    body_start: int|None
    body_end: int|None
    text: str
    @property
    def lines(self):return self.text.count('\n')+1


def parse(s):
    ts=tokens(s);pairs=brackets(ts);items=[];i=0;prev=0
    if not ts:return s,[]
    # preserve leading module documentation/header comments in place
    header=s[:ts[0].start];prev=ts[0].start
    while i<len(ts):
        first=i;attrs=[]
        while i<len(ts) and ts[i].text=='#':
            a=i;i+=1
            if ts[i].text=='!':
                i+=1
                assert ts[i].text=='['
                i=pairs[i]+1
                items.append(Item(prev,ts[a].start,ts[i-1].end,'inner_attribute','',ts[a].start,ts[a].start,ts[a].start,'',None,None,s[prev:ts[i-1].end]))
                prev=ts[i-1].end;first=i
                if i==len(ts):break
            else:
                assert ts[i].text=='[',ts[i]
                attrs.append((a,pairs[i]));i=pairs[i]+1
        if i==len(ts):break
        sig=i;vis_start=ts[i].start;visibility=''
        if ts[i].text=='pub':
            a=i;i+=1
            if ts[i].text=='(':i=pairs[i]+1
            visibility=s[ts[a].start:ts[i-1].end]
        vis_end=ts[i].start if visibility else vis_start
        k=i
        while ts[k].text in ('unsafe','async','default','auto'):
            k+=1
        if ts[k].text=='const' and ts[k+1].text in ('unsafe','async','fn'):
            k+=1
            while ts[k].text in ('unsafe','async'):k+=1
        if ts[k].text=='extern' and ts[k+1].text.startswith('"') and ts[k+2].text=='fn':k+=2
        kind=ts[k].text
        if kind not in ('fn','struct','enum','union','trait','impl','mod','use','const','static','type','extern','macro_rules','macro'):
            kind='macro_call'
        name=''
        if kind in ('fn','struct','enum','union','trait','mod','const','static','type','macro_rules','macro'):
            off=2 if kind=='macro_rules' else 1
            if kind=='static' and ts[k+1].text=='mut':off=2
            name=ts[k+off].text
        elif kind=='macro_call':name=ts[k].text
        j=i;body_start=body_end=None
        while j<len(ts):
            t=ts[j].text
            if t==';':end_i=j+1;break
            if t in ('{','(','['):
                close=pairs[j]
                if t=='{' and kind in ('fn','struct','enum','union','trait','impl','mod','extern','macro_rules','macro','macro_call'):
                    body_start=ts[j].end;body_end=ts[close].start
                    end_i=close+1
                    if end_i<len(ts) and ts[end_i].text==';':end_i+=1
                    break
                j=close+1
            else:j+=1
        else:raise ValueError(('unterminated item',kind,name,ts[i]))
        end=ts[end_i-1].end
        item=Item(prev,ts[first].start,end,kind,name,ts[sig].start,vis_start,vis_end,visibility,body_start,body_end,s[prev:end])
        items.append(item);prev=end;i=end_i
    # don't drop trailing comments or whitespace
    if items:items[-1].text+=s[prev:];items[-1].end=len(s)
    return header,items

def norm(s):return tuple(t.text for t in tokens(s))

