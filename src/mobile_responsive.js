// from https://github.com/alefnode/ubports-apps/blob/master/whatsweb/app/ubuntutheme.js

// ==UserScript==
// @name          WhatsApp (Responsive mode)
// @description   WhatsApp web is now responsive
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
const X = {
  app: () => document.querySelector("#app"),
  browser: () => document.getElementById('app').getElementsByClassName('browser')[0] ,
  
  //MainWrapper stuff (element class two)----------------------------------------------------
  mainWrapper: () => document.querySelector('.two'),  
    unkownSection1: () => document.querySelector('.two').childNodes[2],
      unkownSection2: () => document.querySelector('.two').childNodes[2].childNodes[0],  
    overlayMenus: () => document.querySelector('.two').childNodes[3],
      uploadPannel: () => document.querySelector('.two').childNodes[3].childNodes[1], //(to upload photos/videos/document)    
      leftSettingPannel: () => document.querySelector('.two').childNodes[3].childNodes[0], // leftMenus (Settings, status, community, profile, ...)
    chatList: () => document.querySelector('.two').childNodes[4],
      chatListHeader: () => document.querySelector('.two').childNodes[4].querySelector('header').querySelector('header'),
    chatWindow: () => document.querySelector('.two').childNodes[5],
      chatHeader: () => document.querySelector('.two').childNodes[5].querySelector('header'),
    contactInfo: () => document.querySelector('.two').childNodes[6],
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
  
  
  linkedDevicesInstructions: () => document.querySelector('#link-device-instructions-list'),
  loginView: () => document.querySelector('#link-device-instructions-list').parentElement.parentElement.parentElement.parentElement.parentElement,
  
  
  //-----------------------------------------------------------------------------------------
  isInCommunityPannel: () => (document.querySelector("[role=navigation]") != null),
  isElementInChatlist: (el) => ( el.closest('[role="grid"]')!= null ),
  isElementChatOpenerInCommunityPanel: (el) => X.leftSettingPannel().contains(lastClickEl) && lastClickEl.closest('[role="listitem"]') && lastClickEl.closest('[role="listitem"]').querySelector("[title]"),
  isAPossibleChatOpener: (el) => (el.closest("[role=listitem]") != null)
};

//----------------------------------------------s--------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// SECTION2:   Main() function that is called when it is detected that the main view has loaded
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//------------------------------------------------------------------------------------------------
function main(){
  console.log("Call main function")
  
  // //Adapt fontsize
     try {
        addCss(".customDialog { transform: scaleX(0.8) scaleY(0.8) !important; transition: transform 0.3s ease !important; }");    
        addCss(".emojiDialog { transform: scaleX(0.7) scaleY(0.7) !important; transition: transform 0.3s ease !important; transformOrigin = left bottom !important; left:2% !important; }");      
        addCss(".message-out {  padding-right: 20px !important; }");
        addCss(".message-in {  padding-left: 20px !important; }");  
        addCss("span { font-size: "+window.appConfig.spanFontSize+"% !important; }");    
        addCss(".copyable-text { font-size: "+window.appConfig.textFontSize+"% !important; }");         
        addCss(".html-span { font-size: 96% !important; }");
    } catch (e) { console.log("Error while applying css: "+e) }

  
  X.overlayMenus().style.width="0";
  showchatlist();  
  X.chatList().style.minWidth = "100%"
  X.chatWindow().style.minWidth = "100%" 
  X.chatWindow().style.maxWidth = "100%"  
  X.chatWindow().style.width = "100%"   
  X.mainWrapper().style.minWidth = 'auto';
  X.mainWrapper().style.minHeight = 'auto';
  X.unkownSection1().style.borderInlineStartWidth = "0" ;
  
  // Handle contactInfo Openned panel
  if (X.contactInfo() !== undefined){
        inchatcontactandgroupinfo();
  }
      
   //--------------------------------------------------------------
   // SECTION2.1 Avoid opening the keyboard when entering a chat
  //              by listening to focusin
  //---------------------------------------------------------------
  document.body.addEventListener('focusin', (event) => {
    lastFocusEl = event.target;
    if ( lastFocusEl.isContentEditable  && (!lastClickEl || ! lastClickEl.isContentEditable ) )
    {
      lastFocusEl.blur();
      lastFocusEl.setAttribute('contenteditable', false);
      lastFocusEl.classList.add('contenteditableDisabled');
    }
    
    if (X.chatWindow().contains(lastFocusEl))
    {
      calculateSecondaryChatWindowOpen();
    }
    
  });

  addLeftMenuButtonToChatList();
  
   if (X.leftMenu()) {
     X.leftMenu().style.display = 'none';
   }
    
  //-------------------------------------
  //SECTION2.2   Fix emoticons panel
  //-------------------------------------
  if (X.smileyWrapper()) {
    const observer = new MutationObserver((mutationsList) => {
          X.smileyPanel().style.transformOrigin = "left bottom";
          X.smileyPanel().classList.add('emojiDialog') 
    });
    observer.observe(X.smileyWrapper(), { childList: true, subtree: true });
  }
  
  //------------------------------------------------------------
  //SECTION2.3 Open left panel when changes are detected in it
  //------------------------------------------------------------
  if (X.leftSettingPannel()) {
    setTimeout( () => {
    const observer = new MutationObserver((mutationsList) => {
          if ( X.leftMenu().style.display == 'none' && X.chatList().style.left != "-100%" 
            && !(lastClickEl != null &&  X.isAPossibleChatOpener(lastClickEl) &&  ! X.app().contains(lastClickEl)  )  )
          {
              console.log("toggle menu")
              toggleLeftMenu();
          }
    });
    observer.observe(X.leftSettingPannel(), { childList: true, subtree: true });
    },35)
  }
  
  
  //Send theme information to mainView
  console.log("[ThemeBackgroundColorDebug]"+getComputedStyle(X.leftMenu()).getPropertyValue('--WDS-surface-default').trim());

  
  //------------------------------------------------------------
  //SECTION2.4 global mutation observer
  //------------------------------------------------------------
  const observer3 = new MutationObserver((mutations, obs) => {
    
    if (X.dialog())
    {
      X.dialog().style.minWidth="100%"
      X.dialog().firstChild.classList.add('customDialog')
    }
    
    backupBackButton()
    
  });
  // Observe the whole body
  observer3.observe(document.body, {
    childList: true,
    subtree: true
  });  
  
  //Request by default webnofications permission
  Notification.requestPermission();
}


//-----------------------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//  SECTION3:   Click handler: this allows to intercept any click made by the user and do
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//-----------------------------------------------------------------------------------------

window.addEventListener("click", function() {
  //Register Last clicked element
  lastClickEl=event.target;  
  
   //---------------------------------------------------------------------------------
   //!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
   // Important section: Handle navigation towards chatWindow
   //!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
  //----------------------------------------------------------------------------------  
  if (X.isElementInChatlist(lastClickEl))
        showchatWindow();
  
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
  }
  if ( lastClickEl.querySelector('.contenteditableDisabled') !== null  )
  {
    var editableEl=lastClickEl.querySelector('.contenteditableDisabled');
    lastClickEl.querySelector('.contenteditableDisabled').setAttribute('contenteditable', true);
    lastClickEl.querySelector('.contenteditableDisabled').classList.remove('contenteditableDisabled') 
    editableEl.focus();
  }
  },5);
  
}); 


function calculateSecondaryChatWindowOpen()
{
  if ( X.isInCommunityPannel() )
  {
  //Special detect for in-community Panel
    if (X.isElementChatOpenerInCommunityPanel(lastClickEl))
        showchatWindow();
  }
  else
  {
    //If the focus was requested to ChatWindow
    if (X.chatWindow().contains(lastFocusEl) )
    {
      //Click form Settings Panel (except community panel) -> proceed and open the chatWindow
        if( X.leftSettingPannel().contains(lastClickEl) )
        {
        //We have clicked on an element of left window
        showchatWindow();
        }
        //If we have a click on an orpheline listitem proceed as well
        else if ( ! X.app().contains(lastClickEl) && X.isAPossibleChatOpener(lastClickEl) )
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
  
 if ( X.leftMenu().style.display != 'none')
   toggleLeftMenu()
  
  //Slide back Chatlist panel to main view  
  X.chatList().style.transition= "left 0.25s ease-in-out";
  X.chatList().style.position= 'absolute';
  X.chatList().style.left= '0'; 
  
  
  document.querySelectorAll(".contenteditableDisabled").forEach(el2 => {
    el2.classList.remove('contenteditableDisabled') 
    el2.setAttribute("contenteditable", "true");
  });
}

function showchatWindow(){
  //Make sure to unfocus any focused élément of previous view
   document.activeElement.blur();
   
   X.chatWindow().style.position=""
   X.chatWindow().style.left=""
   X.chatWindow().style.minWidth = "100%" 
   X.chatWindow().style.maxWidth = "100%"  
   X.chatWindow().style.width = "100%"
   
   //Slide Chatlist panel to the left
   X.chatList().style.transition= "left 0.25s ease-in-out";
   X.chatList().style.position= 'absolute'; 
   X.chatList().style.left= "-100%";
   
  //Hide left menu (in case it was oppened)
   X.leftMenu().style.display = 'none';
   X.unkownSection2().style.minWidth = "100%"    
   X.overlayMenus().style.minWidth = "100%"
   X.overlayMenus().style.width="100%"; 
   
   //Activate Upload Panel, in case the user will upload some files
    X.uploadPannel().style.width="100%";
    X.uploadPannel().style.minWidth="100%";   
    X.leftSettingPannel().style.display="none"; 
    
    // Handle contactInfo Openned panel
    if (X.contactInfo() !== undefined){
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
 if (X.chatList().style.left== "-100%") {
  if ( X.mainDiv() && X.chatHeader() )
  {
    if (! X.chatHeader().querySelector('#back_button') )
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
  
    if (  X.chatListHeader() && X.chatListHeader().firstChild && ! X.chatListHeader().querySelector('#added_menu_button') )
    {
    addCss(".added_menu_button span { display:block; height: 100%; width: 100%;}.added_menu_button {  z-index:500; width:50px; height:45px; } html[dir] .added_menu_button { border-radius:50%; } html[dir=ltr] .added_menu_button { right:11px } html[dir=rtl] .added_menu_button { left:11px } .added_menu_button path { fill:var(--panel-header-icon); fill-opacity:1 } .svg_back { transform: rotate(90deg); height: 100%;}");

    var newHTML         = document.createElement('div');
    newHTML.className += "added_menu_button";
    newHTML.style = "";
    newHTML.addEventListener("click", toggleLeftMenu);    
    newHTML.innerHTML   = '<a href="javascript:void(0);" ><span class="html-span" style="height:50px; width:60px;"><div class="html-div" style="padding:10px; --x-transform: none;"><div aria-expanded="false" aria-haspopup="menu" aria-label="MenuLeft" class=""><div class="html-div"><span aria-hidden="true" data-icon="more-refreshed" ><svg viewBox="0 0 24 24" height="24" width="24" preserveAspectRatio="xMidYMid meet" class="" fill="none"><title>more-refreshed</title><path d="M12 20C11.45 20 10.9792 19.8042 10.5875 19.4125C10.1958 19.0208 10 18.55 10 18C10 17.45 10.1958 16.9792 10.5875 16.5875C10.9792 16.1958 11.45 16 12 16C12.55 16 13.0208 16.1958 13.4125 16.5875C13.8042 16.9792 14 17.45 14 18C14 18.55 13.8042 19.0208 13.4125 19.4125C13.0208 19.8042 12.55 20 12 20ZM12 14C11.45 14 10.9792 13.8042 10.5875 13.4125C10.1958 13.0208 10 12.55 10 12C10 11.45 10.1958 10.9792 10.5875 10.5875C10.9792 10.1958 11.45 10 12 10C12.55 10 13.0208 10.1958 13.4125 10.5875C13.8042 10.9792 14 11.45 14 12C14 12.55 13.8042 13.0208 13.4125 13.4125C13.0208 13.8042 12.55 14 12 14ZM12 8C11.45 8 10.9792 7.80417 10.5875 7.4125C10.1958 7.02083 10 6.55 10 6C10 5.45 10.1958 4.97917 10.5875 4.5875C10.9792 4.19583 11.45 4 12 4C12.55 4 13.0208 4.19583 13.4125 4.5875C13.8042 4.97917 14 5.45 14 6C14 6.55 13.8042 7.02083 13.4125 7.4125C13.0208 7.80417 12.55 8 12 8Z" fill="currentColor"></path></svg></span></div><div class="html-div" role="none" data-visualcompletion="ignore" style="inset: 0px;"></div></div></div></span></a>';
    
    //Insert it, TODO improve the way it is inserted
          X.chatListHeader().firstChild.style.width="calc(100% - 40px)";
          X.chatListHeader().prepend(newHTML); 
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

    if (! X.chatHeader().querySelector('#back_button') )
        X.chatHeader().prepend(newHTML);
}

//------------------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//  SECTION6:        Function To display or hide left menu
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//------------------------------------------------------------------------------------
function toggleLeftMenu(){
  if (X.leftMenu()) {
      if ( X.leftMenu().style.display == 'none' )
      {
        X.leftMenu().style.display = 'block';
        X.unkownSection2().style.minWidth = "90%"
        X.chatList().style.left= '';
        X.chatList().style.position= 'static';

        X.overlayMenus().style.width="100%";
        X.overlayMenus().style.minWidth = "90%"
        
        
        X.uploadPannel().style.width="";
        X.uploadPannel().style.minWidth="";   
        X.leftSettingPannel().style.display="";
        X.leftSettingPannel().style.maxWidth="85%";            
        X.leftSettingPannel().style.minWidth="85%";  
        X.chatWindow().style.position="absolute"
        X.chatWindow().style.left="0"
        X.leftMenu().style.marginRight="-1px"
        
      }
      else
      {
        X.chatWindow().style.position=""
        X.chatWindow().style.left=""
        X.chatList().style.position= 'absolute';
        X.chatList().style.left= '0';
        X.overlayMenus().style.minWidth = "0%"
        X.overlayMenus().style.width="0%";
        setTimeout(() => {
           X.leftMenu().style.display = 'none';
           X.unkownSection2().style.minWidth = "100%"   
        }, 500);
        //Send theme information to mainView when closing menus
          console.log("[ThemeBackgroundColorDebug]"+getComputedStyle(X.leftMenu()).getPropertyValue('--WDS-surface-default').trim());
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
    if (X.landingWrapper()) {
      X.landingWrapper().style.minWidth = 'auto';
      X.landingHeader().style.display = 'none';
    }
    if (X.linkedDevicesInstructions())
    {
      //Make the login page responsive
      X.loginView().style.width="100%"
      X.loginView().style.height="100%"
      X.loginView().style.position="fixed"
      X.loginView().style.left="0"
      X.loginView().style.top="0"
      X.loginView().style.borderRadius= "0";
      X.loginView().style.paddingLeft= "5%";
      X.linkedDevicesInstructions().parentElement.parentElement.style.transformOrigin="left";
      X.linkedDevicesInstructions().parentElement.parentElement.style.transform="scaleX(0.8) scaleY(0.8)";
      console.log("[HideAppControls]")
    }
    if (X.mainWrapper().childNodes.length) {
      if ( check == 0 ) {
        clearInterval(checkExist);
        console.log("[HideAppControls]")
        main();
        check = 1;
      }
    }
}, 1000);

//----------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//          SECTION9:  function to handle contactInfo pannel
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//----------------------------------------------------------------------------

function inchatcontactandgroupinfo(){
  if (X.contactInfo()){
      //We need for this section to use absolute postion
      X.contactInfo().style.position= "absolute";
      X.contactInfo().style.width = "100%";
      X.contactInfo().style.maxWidth = "100%";  
      X.contactInfo().style.pointerEvents="none";
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
    console.log(event.target.readyState);
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



//----------------------------------------------------------------------------
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//   SECTION13:  Handle blob downloads Workaround. 
//               This work with qml-download-helper-module to allow downloads
//               Despite that Qt5 does not support download from blobs.
//               TO BE REMOVED WHEN UPGRADING TO QT6
//!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
//----------------------------------------------------------------------------

const blobMap = new Map();
var downloadedBlob;

  // 1) Surveiller la création des blob: URLs
  const origCreateObjectURL = URL.createObjectURL.bind(URL);
  URL.createObjectURL = function (blob) {
    const url = origCreateObjectURL(blob);
    try {
      blobMap.set(url, { blob, createdAt: new Date() });
    } catch (e) { /* fail silently si Map non permise */ }
    return url;
  };

  // 1b) Surveiller revoke (nettoyage)
  const origRevokeObjectURL = URL.revokeObjectURL.bind(URL);
  URL.revokeObjectURL = function (url) {
    if (blobMap.has(url)) {
      blobMap.delete(url);
    }
    return origRevokeObjectURL(url);
  };


  
  function saveBlob(blob, key) {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open("MyDB", 1);

        request.onupgradeneeded = (event) => {
            const db = event.target.result;
            if (!db.objectStoreNames.contains("blobs")) {
                db.createObjectStore("blobs");
            }
        };

        request.onsuccess = (event) => {
            const db = event.target.result;
            const tx = db.transaction("blobs", "readwrite");
            const store = tx.objectStore("blobs");
            store.put(blob, key);

            tx.oncomplete = () => {resolve(); console.log('[DownloadBlob] test');}
            tx.onerror = (e) => reject(e);
        };

        request.onerror = (e) => reject(e);
    });
}

  // 2) Intercepter les clics sur les liens <a> pointant vers blob:
  document.addEventListener('click', function (ev) {
    // ne pas empêcher le comportement par défaut, juste logger
    let target = ev.target;
    while (target && target !== document) {
      if (target.tagName === 'A' && target.href) {
        try {
          const href = target.href;
          if (href.startsWith('blob:')) {
            const entry = blobMap.get(href);
            downloadedBlob=entry;
            saveBlob(downloadedBlob.blob,"testpierre")
          }
        } catch (e) { /* ignore */ }
        break; // qu'on trouve ou pas, on sort
      }
      target = target.parentNode;
    }
  }, true); // capture phase pour attraper tôt