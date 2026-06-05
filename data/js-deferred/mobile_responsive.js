// ==UserScript==
// @name          WhatsApp (Responsive mode)
// @description	  WhatsApp web is now responsive
// @authors       Adrian Campos Garrido, Pierre Parent
// @version       20251009
// @include       https://*.whatsapp.com/*
// ==/UserScript==

//Here is the code structure:
//
//SECTION1:   Layer of abstraction for WhatsApp web page, allows to manipulate objects with clear names
//            If something breaks because of a change upstream that's where you want to start investigating
//
//SECTION2:   Main() function that is called when it is detected that the main view has loaded
//            It sets everything up. See subsections bellow
//
//SECTION3:   Click handler: this allows to intercept any click made by the user and do
//
//SECTION4:   Navigation functions showchatWindow() and showchatList() to switch from chatview to chatlist
//
//SECTION5:   Functions to add navigation buttons to headers (back button to go back to chatlist and leftmenu button)
//
//SECTION6:   Function To display or hide left menu
//
//SECTION7:   Code for Quick copy to ClipBoard
//
//SECTION8:   Pre-Loader: this code executes before the mainview is started. Its role is to detect when the mainview is started
//            And to make responsive anything that displays before the mainview
//
//SECTION9:  function to handle contactInfo pannel
//
//SECTION10:  Declare global variables and useful functions
//
//SECTION11:  Request Desktop Notification permission, on load
//
//SECTION12:  Detect Audio évents to trigger Notifications
//                to detect audio notifications
//
//SECTION13:  Handle blob downloads Workaround. 
//               This work with qml-download-helper-module to allow downloads
//               Despite that Qt5 does not support download from blobs.


//SECTION2 subsections:

//SECTION2.1 Avoid opening the keyboard when entering a chat
//              by listening to focusin
//
//SECTION2.2 Fix emoticons panel
//
//SECTION2.3 Open left panel when changes are detected in it
//
//SECTION2.4 global mutation observer


//---------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//  SECTION1:   Layer of abstraction for WhatsApp web page
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//---------------------------------------------------------------


var indexChatList=4;

const My= {
  app: () => document.querySelector("#app"),
  browser: () => document.getElementById('app').getElementsByClassName('browser')[0] ,
  
  //MainWrapper stuff (element class two)----------------------------------------------------
  mainWrapper: () => document.querySelector('.two'),  
    unkownSection1: () => document.querySelector('.two').childNodes[indexChatList-2],
      unkownSection2: () => document.querySelector('.two').childNodes[indexChatList-2].childNodes[0],  
    overlayMenus: () => document.querySelector('.two').childNodes[indexChatList-1],
      uploadPannel: () => document.querySelector('.two').childNodes[indexChatList-1].childNodes[1], //(to upload photos/videos/document)    
      leftSettingPannel: () => document.querySelector('.two').childNodes[indexChatList-1].childNodes[0], // leftMenus (Settings, status, community, profile, ...)
    chatList: () => document.querySelector('.two').childNodes[indexChatList],
      chatListHeader: () => document.querySelector('.two').childNodes[indexChatList].querySelector('header').querySelector('header'),
    chatWindow: () => document.querySelector('.two').childNodes[indexChatList+1],
      chatHeader: () => document.querySelector('.two').childNodes[indexChatList+1].querySelector('header'),
    contactInfo: () => document.querySelector('.two').childNodes[indexChatList+3],
  //-------------------------------------------------------------------------------------------

  upperWrapper: () => document.querySelector('.three'),
      
  leftMenu: () => document.querySelector('header'),

  
  smileyWrapper: () => document.getElementById('expressions-panel-container'),
  smileyPanel: () => document.querySelector('#expressions-panel-container > :first-child > :first-child'),
  
  newChatButton: () => document.querySelector('[data-icon="new-chat-outline"]').parentElement.parentElement,
  archivedChatButton: () => document.querySelector('#pane-side').childNodes[0], 
  
  //Landing elements (Only present temporarilly while whatsapp is loading)
  landingWrapper: () => document.querySelector('.landing-wrapper'),
  landingHeader: () => document.querySelector('.landing-header'),
  mainDiv: () =>  document.querySelector("div#main"),
  chatHeader: () =>  document.querySelector("div#main").querySelector("header"),
  
  dialog: () =>document.querySelector('[role="dialog"]'),
  callsDialog: ()  =>document.querySelector('#wa-popovers-bucket').firstChild, 
  
  
  linkedDevicesInstructions: () => document.querySelector('#link-device-instructions-list'),
  loginView: () => document.querySelector('#link-device-instructions-list').parentElement.parentElement.parentElement.parentElement.parentElement,
  
  
  //-----------------------------------------------------------------------------------------
  isInCommunityPannel: () => (document.querySelector("[role=navigation]") != null),
  isElementInChatlist: (el) => ( el.closest('[role="grid"]')!= null ),
  isElementChatOpenerInCommunityPanel: (el) => My.leftSettingPannel().contains(lastClickEl) && lastClickEl.closest('[role="listitem"]') && lastClickEl.closest('[role="listitem"]').querySelector("[title]"),
  isAPossibleChatOpener: (el) => (el.closest("[role=listitem]") != null)
};


function findIndexChatList()
{
  const parent = document.querySelector('.two');
indexChatList =
  parent &&
  (() => {
    const grid = parent.querySelector('[role="grid"]');
    if (!grid) return -1;

    let child = grid;

    while (child.parentElement !== parent) {
      child = child.parentElement;
      if (!child) return -1;
    }

    return Array.from(parent.children).indexOf(child) ;
  })();
  console.log("Index Chat List:"+indexChatList);
}


//----------------------------------------------s--------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// SECTION2:   Main() function that is called when it is detected that the main view has loaded
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//------------------------------------------------------------------------------------------------
function main(){
  console.log("Call main function")
  
  My.overlayMenus().style.width="0";
  showchatlist();  
  My.chatList().style.minWidth = "100%"
  My.chatWindow().style.minWidth = "100%" 
  My.chatWindow().style.maxWidth = "100%"  
  My.chatWindow().style.width = "100%"   
  My.mainWrapper().style.minWidth = 'auto';
  My.mainWrapper().style.minHeight = 'auto';
  My.unkownSection1().style.borderInlineStartWidth = "0" ;
  My.chatList().style.paddingLeft="0px"
  
  // Handle contactInfo Openned panel
  if (My.contactInfo() !== undefined){
        inchatcontactandgroupinfo();
  }
      


  addLeftMenuButtonToChatList();
  
   if (My.leftMenu()) {
     My.leftMenu().style.display = 'none';
   }
    
  //-------------------------------------
  //SECTION2.2   Fix emoticons panel
  //-------------------------------------
  if (My.smileyWrapper()) {
    const observer = new MutationObserver((mutationsList) => {
          My.smileyPanel().style.transformOrigin = "left bottom";
          My.smileyPanel().classList.add('emojiDialog') 
    });
    observer.observe(My.smileyWrapper(), { childList: true, subtree: true });
  }
  
  //------------------------------------------------------------
  //SECTION2.3 Open left panel when changes are detected in it
  //------------------------------------------------------------
  if (My.leftSettingPannel()) {
    setTimeout( () => {
    const observer = new MutationObserver((mutationsList) => {
          if ( My.leftMenu().style.display == 'none' && My.chatList().style.transform != "translateX(-100%)" 
            && !(lastClickEl != null &&  My.isAPossibleChatOpener(lastClickEl) &&  ! My.app().contains(lastClickEl)  )  )
          {
              console.log("toggle menu")
              toggleLeftMenu();
          }
    });
    observer.observe(My.leftSettingPannel(), { childList: true, subtree: true });
    },35)
  }

  My.chatList().classList.add("NavSidebar");
     
  //Send theme information to mainView
  console.log("[ThemeBackgroundColorDebug]"+getComputedStyle(My.leftMenu()).getPropertyValue('--WDS-surface-default').trim());

  //Request by default webnofications permission
  Notification.requestPermission();

}


  
  // //Adapt fontsize
     try {
       //Handle lock screen
        addCss('main { width: 100% !important; height: 100%; padding: 0 !important; position: fixed; left:0; top: 0; border-radius:0 !important; padding-left:5%; } '); 
        addCss(".customDialog { transform: scaleX(0.8) scaleY(0.8) !important; transition: transform 0.3s ease !important; }");
        addCss('.customDialog:has([direction="vertical"]) { transform: scaleX(0.55) scaleY(0.55) !important; padding-top: 5% !important; padding-left: 5% !important; height: 180% !important; }');
         addCss('[data-animate-modal-body="true"]:has([direction="vertical"]) > * { height: 100% !important; } ');
        addCss(".emojiDialog { transform: scaleX(0.66) scaleY(0.66) !important; transition: transform 0.3s ease !important; transformOrigin = left bottom !important; left:2% !important; }"); 
        addCss(".NavSidebar { transition: transform 0.25s ease-in-out !important }")
        addCss(".message-out {  padding-right: 20px !important; }");
        addCss(".message-in {  padding-left: 20px !important; }");  
        addCss("span { font-size: "+107+"% !important; }");    
        addCss(".copyable-text { font-size: "+106+"% !important; }");         
        addCss(".html-span { font-size: 96% !important; }");
        addCss('[data-animate-dropdown-item="true"] { left: 2vw !important ; } ');
    } catch (e) { console.log("Error while applying css: "+e) }

  // Listener prioritaire pour Enter
document.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    if( My.leftSettingPannel().contains(lastClickEl) && lastClickEl.isContentEditable )
    {
    lastClickEl=null;
    console.log("Stoping Enter propagation");
    e.stopImmediatePropagation(); 
    let text = lastFocusEl.innerText || lastFocusEl.textContent;
    if ( text.charAt(text.length - 1) === " " )
    {
              console.log("clean Search");
              sent=0;
              moveCursorRight()    
                lastFocusEl.dispatchEvent(new KeyboardEvent('keydown', {
                     key: 'Backspace',
                     code: 'Backspace',
                     bubbles: true
                 }));
                
      }
    }
    console.log('Enter pressed, blur !');
    lastFocusEl.blur();
  }
}, true); // <-- "true" pour écouter en phase capture (avant les listeners normaux)



   //--------------------------------------------------------------
   // SECTION2.1 Avoid opening the keyboard when entering a chat
  //              by listening to focusin
  //---------------------------------------------------------------
  document.body.addEventListener('focusin', (event) => {
    lastFocusEl = event.target;
    
    if ( lastFocusEl.isContentEditable )
    {
        var sent = 0;
        var timeout;
        
        if ( editObserver != null )
        editObserver.disconnect();

        editObserver = new MutationObserver(() => {
          clearTimeout(timeout);

          timeout = setTimeout(() => {
            const editableElement = lastFocusEl;
            let text = editableElement.innerText || editableElement.textContent;
            if ( ! text.includes(' ') &&  text.trim().length > 0 && sent ==0 )
            {
              console.log("Add space at the end");
              document.execCommand("insertText", false, " ");
              sent=1
            }
            if ( text.trim() === '' && sent === 1 )
            {
              console.log("clean 1");
              sent=0;
              moveCursorRight()    
                editableElement.dispatchEvent(new KeyboardEvent('keydown', {
                     key: 'Backspace',
                     code: 'Backspace',
                     bubbles: true
                 }));
                
            }
               
          }, 100);
        });

        editObserver.observe(lastFocusEl, {
          childList: true,
          subtree: true,
          characterData: true,
          attributes: true
        }); 
      
    }
    
    if ( lastFocusEl.isContentEditable  && (!lastClickEl || ! lastClickEl.isContentEditable ) )
    {
      lastFocusEl.blur();
      lastFocusEl.setAttribute('contenteditable', false);
      lastFocusEl.classList.add('contenteditableDisabled');
      document.querySelector('footer').style.paddingBottom="0";      
    }
    
    if ( ! document.querySelector('footer').contains(lastFocusEl) )
      {
        document.querySelector('footer').style.paddingBottom="0";
      }
    
    if (My.chatWindow().contains(lastFocusEl))
    {
      calculateSecondaryChatWindowOpen();
    }
    
  });
  
   //------------------------------------------------------------
  //SECTION2.4 global mutation observer
  //------------------------------------------------------------
  const observer3 = new MutationObserver((mutations, obs) => {
    
    if (My.dialog())
    {
      My.dialog().style.minWidth="100%"
      My.dialog().firstChild.classList.add('customDialog')
    }
    
    backupBackButton()
    
  });
  // Observe the whole body
  observer3.observe(document.body, {
    childList: true,
    subtree: true
  });  
  


//-----------------------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//  SECTION3:   Click handler: this allows to intercept any click made by the user and do
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//-----------------------------------------------------------------------------------------

function moveCursorRight() {
    const sel = window.getSelection();
    if (!sel.rangeCount) return;

    const range = sel.getRangeAt(0);
    const node = range.startContainer;

    if (node.nodeType === Node.TEXT_NODE) {
        let offset = range.startOffset + 1;
        offset = Math.min(offset, node.textContent.length);

        const newRange = document.createRange();
        newRange.setStart(node, offset);
        newRange.collapse(true);

        sel.removeAllRanges();
        sel.addRange(newRange);
    }
}


var editObserver=null;

document.addEventListener("mousedown", (event) => {
    //---------------------------------------------------------------------------------
   //!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
   // Important section: Handle navigation towards chatWindow
   //!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
  //----------------------------------------------------------------------------------  
  if (My.isElementInChatlist(event.target))
    requestAnimationFrame(() => {
     showchatWindow();
    })
}, true);

window.addEventListener("click", function() {
  //Register Last clicked element
  lastClickEl=event.target;  
  
  if (My.isElementInChatlist(lastClickEl) && My.chatList().style.transform != "translateX(-100%)" )
  {
    showchatWindow();
  }
  
  setTimeout( () => {
  calculateSecondaryChatWindowOpen();
  },5);
  setTimeout( () => {
  //(Re)-enable content Editable ( If it was disabled when "OnFocus" was called without click)
  if ( lastClickEl.closest('.contenteditableDisabled') !== null  )
  {
    var editableEl=lastClickEl.closest('.contenteditableDisabled');
    lastClickEl.closest('.contenteditableDisabled').setAttribute('contenteditable', true);
    lastClickEl.closest('.contenteditableDisabled').classList.remove('contenteditableDisabled') 
    editableEl.focus();
     if ( document.querySelector('footer').contains(lastClickEl) )
      {
        console.log("Add margin");
        try
        {
        var keyboardHeight = Math.round(parseFloat(window.__cmdParams.keyboardHeight) /parseFloat(window.__cmdParams.forceScale));
        document.querySelector('footer').style.paddingBottom=`${keyboardHeight}px`;
        }
        catch (e) { /* safe */ }
      }  
  }
  if ( lastClickEl.querySelector('.contenteditableDisabled') !== null  )
  {
    var editableEl=lastClickEl.querySelector('.contenteditableDisabled');
    lastClickEl.querySelector('.contenteditableDisabled').setAttribute('contenteditable', true);
    lastClickEl.querySelector('.contenteditableDisabled').classList.remove('contenteditableDisabled') 
    editableEl.focus();
  }
  },5);
  
}, true); 


function calculateSecondaryChatWindowOpen()
{
  if ( My.isInCommunityPannel() )
  {
  //Special detect for in-community Panel
    if (My.isElementChatOpenerInCommunityPanel(lastClickEl))
    {
        showchatWindow();
    }
  }
  else
  {
    //If the focus was requested to ChatWindow
    if (My.chatWindow().contains(lastFocusEl) )
    {
      //Click form Settings Panel (except community panel) -> proceed and open the chatWindow
        if( My.leftSettingPannel().contains(lastClickEl) && ! lastClickEl.isContentEditable )
        {
        console.log("test1!!!!!!!!!!!!");
        //We have clicked on an element of left window
        showchatWindow();
        }
        //If we have a click on an orpheline listitem proceed as well
        else if ( ! My.app().contains(lastClickEl) && My.isAPossibleChatOpener(lastClickEl) )
        {
          showchatWindow();
        }
    }
  }
}

window.addEventListener("click", function() {
  setTimeout( () => {
  backupBackButton();
  },400);
  //Backup Back button
  setTimeout( () => {
  backupBackButton();
  },4000);
})

//---------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//SECTION4:   Navigation functions showchatWindow() and showchatList()
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//---------------------------------------------------------------------
function showchatlist(){
  
 if ( My.leftMenu().style.display != 'none')
   toggleLeftMenu()
  
  //Slide back Chatlist panel to main view  
  My.chatList().style.transition= "transform 0.25s ease-in-out !important";
  My.chatList().style.position= 'absolute';
  My.chatList().style.transform= 'translateX(0)'; 
  
  
  document.querySelectorAll(".contenteditableDisabled").forEach(el2 => {
    el2.classList.remove('contenteditableDisabled') 
    el2.setAttribute("contenteditable", "true");
  });
}

function showchatWindow(){
  //Make sure to unfocus any focused élément of previous view
   document.activeElement.blur();
   
   My.chatWindow().style.position=""
   My.chatWindow().style.left=""
   My.chatWindow().style.minWidth = "100%" 
   My.chatWindow().style.maxWidth = "100%"  
   My.chatWindow().style.width = "100%"
   
   //Slide Chatlist panel to the left
   My.chatList().style.transition= "transform 0.25s ease-in-out !important";
   My.chatList().style.position= 'absolute'; 
   My.chatList().style.transform= "translateX(-100%)";
   
  //Hide left menu (in case it was oppened)
   My.leftMenu().style.display = 'none';
   My.unkownSection2().style.minWidth = "100%"    
   My.overlayMenus().style.minWidth = "100%"
   My.overlayMenus().style.width="100%"; 
   
   //Activate Upload Panel, in case the user will upload some files
    My.uploadPannel().style.width="100%";
    My.uploadPannel().style.minWidth="100%";   
    My.leftSettingPannel().style.display="none"; 
    
    // Handle contactInfo Openned panel
    if (My.contactInfo() !== undefined){
        inchatcontactandgroupinfo();
    }
  
    addBackButtonToChatViewWithTimeout();
}

function addBackButtonToChatViewWithTimeout()
{
      //Add back Button
    setTimeout(() => {
        addBackButtonToChatView();
    }, 20);

    setTimeout(() => {
    addBackButtonToChatView();
    }, 300);    
    
    setTimeout(() => {
    addBackButtonToChatView();
    }, 1500); 
  
}

function backupBackButton()
{
 if (My.chatList().style.transform== "translateX(-100%)") {
  if ( My.mainDiv() && My.chatHeader() )
  {
    if (! My.chatHeader().querySelector('#back_button') )
    {
    addBackButtonToChatView();  
    }
  }
  else
  {
    showchatlist()
  }
} 
}


//---------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//  SECTION5:   Functions to add navigation buttons to headers 
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//---------------------------------------------------------------

//------------------------------------------------------------------------------------
//          Function do add a button to access left menu
//                 inside main chat list header
//------------------------------------------------------------------------------------
function addLeftMenuButtonToChatList(){
  
    if (  My.chatListHeader() && My.chatListHeader().firstChild && ! My.chatListHeader().querySelector('.added_menu_button') )
    {
    addCss(".added_menu_button span { display:block; height: 100%; width: 100%;}.added_menu_button {  z-index:500; width:50px; height:45px; } html[dir] .added_menu_button { border-radius:50%; } html[dir=ltr] .added_menu_button { right:11px } html[dir=rtl] .added_menu_button { left:11px } .added_menu_button path { fill:var(--panel-header-icon); fill-opacity:1 } .svg_back { transform: rotate(90deg); height: 100%;}");

    var newHTML         = document.createElement('div');
    newHTML.className += "added_menu_button";
    newHTML.style = "";
    newHTML.addEventListener("click", toggleLeftMenu);    
    newHTML.innerHTML   = '<a href="javascript:void(0);" ><span class="html-span" style="height:50px; width:60px;"><div class="html-div" style="padding:10px; --x-transform: none;"><div aria-expanded="false" aria-haspopup="menu" aria-label="MenuLeft" class=""><div class="html-div"><span aria-hidden="true" data-icon="more-refreshed" ><svg viewBox="0 0 24 24" height="24" width="24" preserveAspectRatio="xMidYMid meet" class="" fill="none"><title>more-refreshed</title><path d="M12 20C11.45 20 10.9792 19.8042 10.5875 19.4125C10.1958 19.0208 10 18.55 10 18C10 17.45 10.1958 16.9792 10.5875 16.5875C10.9792 16.1958 11.45 16 12 16C12.55 16 13.0208 16.1958 13.4125 16.5875C13.8042 16.9792 14 17.45 14 18C14 18.55 13.8042 19.0208 13.4125 19.4125C13.0208 19.8042 12.55 20 12 20ZM12 14C11.45 14 10.9792 13.8042 10.5875 13.4125C10.1958 13.0208 10 12.55 10 12C10 11.45 10.1958 10.9792 10.5875 10.5875C10.9792 10.1958 11.45 10 12 10C12.55 10 13.0208 10.1958 13.4125 10.5875C13.8042 10.9792 14 11.45 14 12C14 12.55 13.8042 13.0208 13.4125 13.4125C13.0208 13.8042 12.55 14 12 14ZM12 8C11.45 8 10.9792 7.80417 10.5875 7.4125C10.1958 7.02083 10 6.55 10 6C10 5.45 10.1958 4.97917 10.5875 4.5875C10.9792 4.19583 11.45 4 12 4C12.55 4 13.0208 4.19583 13.4125 4.5875C13.8042 4.97917 14 5.45 14 6C14 6.55 13.8042 7.02083 13.4125 7.4125C13.0208 7.80417 12.55 8 12 8Z" fill="currentColor"></path></svg></span></div><div class="html-div" role="none" data-visualcompletion="ignore" style="inset: 0px;"></div></div></div></span></a>';
    
    //Insert it, TODO improve the way it is inserted
          My.chatListHeader().firstChild.style.width="calc(100% - 40px)";
          My.chatListHeader().prepend(newHTML); 
    }
  
    
}



//-----------------------------------------------------------------------------
//         Function to add a back button in chat view header
//              To go back to main chat list view
//----------------------------------------------------------------------------
function addBackButtonToChatView(){

    addCss(".back_button span { display:block; height: 100%; width: 100%;}.back_button {  z-index:200; width:37px; height:45px; } html[dir] .back_button { border-radius:50%; } html[dir=ltr] .back_button { right:11px } html[dir=rtl] .back_button { left:11px } .back_button path { fill:var(--panel-header-icon); fill-opacity:1 } .svg_back { transform: rotate(90deg); height: 100%;}");
    
    var newHTML         = document.createElement('div');
    newHTML.className += "back_button";
    newHTML.style = "";
    newHTML.addEventListener("click", showchatlist);
    newHTML.innerHTML   = "<span data-icon='left' id='back_button' ><svg class='svg_back' id='Layer_1' xmlns='http://www.w3.org/2000/svg' viewBox='0 0 21 21' width='21' height='21'><path fill='#000000' fill-opacity='1' d='M4.8 6.1l5.7 5.7 5.7-5.7 1.6 1.6-7.3 7.2-7.3-7.2 1.6-1.6z'></path></svg></span>";

    if (! My.chatHeader().querySelector('#back_button') )
        My.chatHeader().prepend(newHTML);
}

//------------------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//  SECTION6:        Function To display or hide left menu
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//------------------------------------------------------------------------------------
function toggleLeftMenu(){
  if (My.leftMenu()) {
      if ( My.leftMenu().style.display == 'none' )
      {
        My.leftMenu().style.display = 'block';
        My.unkownSection2().style.minWidth = "90%"
        My.chatList().style.transform= '';
        My.chatList().style.position= 'static';

        My.overlayMenus().style.width="100%";
        My.overlayMenus().style.minWidth = "90%"
        
        
        My.uploadPannel().style.width="";
        My.uploadPannel().style.minWidth="";   
        My.leftSettingPannel().style.display="";
        My.leftSettingPannel().style.maxWidth="85%";            
        My.leftSettingPannel().style.minWidth="85%";  
        My.chatWindow().style.position="absolute"
        My.chatWindow().style.left="0"
        My.leftMenu().style.marginRight="-1px"
        My.chatList().style.paddingLeft="0px"
        
      }
      else
      {
        My.chatWindow().style.position=""
        My.chatWindow().style.left=""
        My.chatList().style.position= 'absolute';
        My.chatList().style.transform= 'translateX(0)';
        My.overlayMenus().style.minWidth = "0%"
        My.overlayMenus().style.width="0%";
        setTimeout(() => {
           My.leftMenu().style.display = 'none';
           My.unkownSection2().style.minWidth = "100%"   
        }, 500);
        //Send theme information to mainView when closing menus
          console.log("[ThemeBackgroundColorDebug]"+getComputedStyle(My.leftMenu()).getPropertyValue('--WDS-surface-default').trim());
      }
  }
}


//-------------------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//                  SECTION7:   Code for Quick ClipBoard
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//-------------------------------------------------------------------------------------
// Ensemble pour garder la trace des div déjà sélectionnées
var copiedMessage1;
var copiedMessage2;

document.addEventListener("touchend", () => {
  if (window.appConfig.enableQuickCopy)
  {
  const selection = window.getSelection();
  const selectedText = selection.toString().trim();
  if (selectedText.length > 0) {
  const node = selection.anchorNode;
  const div = node?.nodeType === 1 ? node.closest("div") : node?.parentElement?.closest("div");
  
 
    if (div && !div.isContentEditable && copiedMessage1!=div&& copiedMessage2!=div) {
          copiedMessage1=div;
          copiedMessage2=div;
          const range = document.createRange();
          range.selectNodeContents(div);
          selection.removeAllRanges();
          selection.addRange(range);
          const originalHTML = div.innerHTML;
          div.querySelectorAll('img').forEach(img => {
              const altText = img.getAttribute('alt') || '';
              const textNode = document.createTextNode(altText);
            img.replaceWith(textNode);
          });          
          console.log("[ClipBoardCopy]" + window.getSelection().toString());
          div.innerHTML=originalHTML;
          selection.removeAllRanges();
    }
  }
  }
});
   
window.addEventListener("click", function() {
  if (window.appConfig.enableQuickCopy)
  {
  setTimeout(function() {
    // Handle events for Quick Copy to Clipboard
    const selection = window.getSelection();
    const selectedText = selection.toString().trim();

    if (selectedText.length === 0) {
      if (copiedMessage1) copiedMessage1 = null;
      else copiedMessage2 = null;
    }
  }, 800); // délai en millisecondes
  }
});

//-------------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//SECTION8:   Pre-Loader: this code executes before the mainview is started.
//         First resize after loading the web 
//    (temporary timeout only running at the begining)
////!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//-------------------------------------------------------------------------------
var check = 0;
var checkExist = setInterval(function() {
    if (My.landingWrapper()) {
      My.landingWrapper().style.minWidth = 'auto';
      My.landingHeader().style.display = 'none';
    }
    if (My.linkedDevicesInstructions())
    {
      //Make the login page responsive
      My.loginView().style.width="100%"
      My.loginView().style.height="100%"
      My.loginView().style.position="fixed"
      My.loginView().style.left="0"
      My.loginView().style.top="0"
      My.loginView().style.borderRadius= "0";
      My.loginView().style.paddingLeft= "5%";
      My.linkedDevicesInstructions().parentElement.parentElement.style.transformOrigin="left";
      My.linkedDevicesInstructions().parentElement.parentElement.style.transform="scaleX(0.8) scaleY(0.8)";
      console.log("[HideAppControls]")
    }
    if (My.mainWrapper().childNodes.length) {
      if ( check == 0 ) {
        clearInterval(checkExist);
        console.log("[HideAppControls]")
        //main();
        check = 1;
      }
    }
}, 1000);


var mainWrapperExisted=false;

// Création du MutationObserver
const observer = new MutationObserver((mutationsList) => {
  for (const mutation of mutationsList) {
    // On regarde uniquement les ajouts et suppressions d'enfants
    if (mutation.type === 'childList') {
      
      if ( My.mainWrapper() !== null && mainWrapperExisted == false )
          findIndexChatList();
      
      if (My.mainWrapper() !== null && mainWrapperExisted == false && ! My.chatListHeader().querySelector('.added_menu_button') ) {
        main();
        
      }
       mainWrapperExisted=My.mainWrapper()!== null;
    }
  }
});

// Configuration du listener
observer.observe(document.body, {
  childList: true, // On observe les ajouts/suppressions d'enfants
  subtree: true    // On observe aussi les descendants, pas seulement le parent direct
});


     
//----------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//          SECTION9:  function to handle contactInfo pannel
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//----------------------------------------------------------------------------

function inchatcontactandgroupinfo(){
  if (My.contactInfo()){
      //We need for this section to use absolute postion
      My.contactInfo().style.position= "absolute";
      My.contactInfo().style.width = "100%";
      My.contactInfo().style.maxWidth = "100%";  
      My.contactInfo().style.pointerEvents="none";
  }
}

//----------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//   SECTION10:  Declare global variables and useful functions
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//----------------------------------------------------------------------------

// Declare variables
updatenotificacion = 0;
allownotification = 0;
var lastClickEl=null;
var lastFocusEl=null;
var firstChatLoad=1;

  function addCss(cssString) {
      var head = document.getElementsByTagName('head')[0];
      var newCss = document.createElement('style');
      newCss.type = "text/css";
      newCss.innerHTML = cssString;
      head.appendChild(newCss);
  }
  
  
// Listeners to startup APP
window.addEventListener("load", function(event) {
    console.log("Loaded");
    main();
});

document.addEventListener('readystatechange', event => {
    if (event.target.readyState === "complete") {
        console.log("Completed");
    }
});


//----------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//   SECTION11:  Request Desktop Notification permission, on load
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//----------------------------------------------------------------------------
Notification.requestPermission();

//-----------------------------------------------------------------------
//                     End of main thing
//-----------------------------------------------------------------------

//----------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//   SECTION12:  Detect Audio évents to trigger Notifications
//                to detect audio notifications
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//----------------------------------------------------------------------------
(function() {
  if (window.__my_audio_hook_installed) return;
  window.__my_audio_hook_installed = true;

  function logAudioEvent(info) {
    try {
      console.log("[DbgAud] " + info);
    } catch (e) { /* safe */ }
  }

  // 1) Intercepter constructeur Audio (alias de HTMLAudioElement)
  try {
    const OrigAudio = window.Audio;
    window.Audio = function(src) {
      const a = new OrigAudio(src);
      // attach listeners to catch play
      a.addEventListener('play', function(){ logAudioEvent((a.currentSrc || a.src || "")); }, {passive:true});
      a.addEventListener('playing', function(){ logAudioEvent((a.currentSrc || a.src || "")); }, {passive:true});
      return a;
    };
    // preserve prototype / static props
    window.Audio.prototype = OrigAudio.prototype;
    Object.getOwnPropertyNames(OrigAudio).forEach(function(k){
      try { if (!(k in window.Audio)) window.Audio[k] = OrigAudio[k]; } catch(e){}
    });
  } catch(e) {}

  // 2) Intercepter HTMLAudioElement / HTMLMediaElement.play
  try {
    const mp = HTMLMediaElement && HTMLMediaElement.prototype;
    if (mp && !mp.__play_hooked__) {
      const origPlay = mp.play;
      mp.__play_hooked__ = true;
      mp.play = function() {
        try {
          const src = this.currentSrc || this.src || "";
          logAudioEvent( src);
        } catch(e){}
        return origPlay.apply(this, arguments);
      }
    }
  } catch(e){}

})();
