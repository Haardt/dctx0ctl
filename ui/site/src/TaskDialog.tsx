import {useEffect,useRef,useState,type ReactNode} from 'react';
export default function TaskDialog({children,onClose,onFocus}:{children:ReactNode;onClose:()=>void;onFocus:(depth:string)=>void}) {
 const ref=useRef<HTMLDialogElement>(null);const [depth,setDepth]=useState('all');
 useEffect(()=>{const dialog=ref.current;dialog?.showModal();return()=>dialog?.close()},[]);
 return <dialog ref={ref} className="task-dialog" aria-label="Task details" onCancel={e=>{e.preventDefault();onClose()}} onClick={e=>{if(e.target===e.currentTarget){const r=e.currentTarget.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)onClose()}}}>
 <header className="task-dialog-header"><strong>Task details</strong><button aria-label="Close task details" autoFocus onClick={onClose}>×</button></header><div className="details-controls"><button className="tag" onClick={()=>onFocus(depth)}>Focus selected</button><label className="depth-slider">Depth <input aria-label="Details focus depth" type="range" min="0" max="6" step="1" value={depth==='all'?6:Number(depth)} onChange={e=>setDepth(e.target.value==='6'?'all':e.target.value)}/><output>{depth==='all'?'All':depth}</output></label></div><div className="task-dialog-body">{children}</div>
 </dialog>;
}
