import Home from "../src/page-contents/Home.bs.js";
import HtmlHeader from "../src/components/HtmlHeader.js";
import Head from "next/head";

export default function Index(props) {
  return (
    <div>
      <HtmlHeader page="Build bigger, ship faster"></HtmlHeader>
      <Head>
        <meta name="description" content="Build bigger, ship faster" />
      </Head>
      <Home {...props} />
    </div>
  );
}