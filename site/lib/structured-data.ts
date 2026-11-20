import { githubUrl, siteUrl } from './layout.shared';

/**
 * The site and the project as schema.org data: the WebSite gives search results the site's name,
 * and the SoftwareSourceCode says what the page is about and where its code lives.
 */
export function structuredData(description: string): string {
  const author = { '@type': 'Person', name: 'Colin McDonnell', url: 'https://github.com/colinhacks' };
  const data = {
    '@context': 'https://schema.org',
    '@graph': [
      { '@type': 'WebSite', '@id': `${siteUrl}/#website`, url: `${siteUrl}/`, name: 'Reylo', description, inLanguage: 'en' },
      {
        '@type': 'SoftwareSourceCode',
        '@id': `${siteUrl}/#software`,
        name: 'Reylo',
        description,
        url: `${siteUrl}/`,
        codeRepository: githubUrl,
        programmingLanguage: ['TypeScript', 'SQL'],
        runtimePlatform: 'Node.js',
        license: 'https://opensource.org/licenses/MIT',
        author,
        isPartOf: { '@id': `${siteUrl}/#website` },
      },
    ],
  };
  // `<` is escaped so that no string in the data can close the script element.
  return JSON.stringify(data).replace(/</g, '\\u003c');
}
